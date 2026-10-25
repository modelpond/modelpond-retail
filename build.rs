use std::collections::BTreeMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context as _;
use minijinja::AutoEscape;
use minijinja::Environment;
use minijinja::Value;
use mp_catalog::Catalog;
use mp_catalog::Price;
use mp_catalog::Provider;
use mp_catalog::VendorModel;
use serde::Serialize;

#[derive(Serialize)]
struct PageContext<'a> {
    page: &'a str,
    year: i32,
    catalog: &'a Catalog,
    home_flagships: Vec<FlagshipView>,
    providers: Vec<ProviderView>,
    flagship_models: Vec<FlagshipView>,
    groups: Vec<GroupView>,
    legend: Vec<LegendView>,
    provider_page: Option<ProviderView>,
    model_page: Option<ModelPageView>,
}

fn main() {
    println!("cargo:rerun-if-changed=content");
    println!("cargo:rerun-if-changed=../catalog/data");
    generate_site().unwrap_or_else(|error| panic!("failed to generate retail site: {error:#}"));
}

fn generate_site() -> anyhow::Result<()> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let content_dir = manifest_dir.join("content");
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("target"));
    let output_dir = target_dir.join("html");
    if output_dir.exists() {
        fs::remove_dir_all(&output_dir)?;
    }
    fs::create_dir_all(&output_dir)?;

    copy_dir(&content_dir.join("static"), &output_dir.join("static"))?;
    copy_catalog_assets(&output_dir.join("static").join("catalog"))?;

    let environment = template_environment(&content_dir.join("templates"))?;
    let catalog = mp_catalog::data();
    let views = build_views(catalog);
    let mut home_flagships = views.flagships.clone();
    home_flagships.sort_by(|left, right| {
        output_cost(left)
            .partial_cmp(&output_cost(right))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.name.cmp(&right.name))
    });
    for (template, output, page) in [
        ("index.html", "index.html", "home"),
        ("instruct.html", "instruct/index.html", "instruct"),
        ("gateway.html", "gateway/index.html", "gateway"),
        ("code.html", "code/index.html", "code"),
        ("batch.html", "batch/index.html", "batch"),
        ("catalogue.html", "catalogue/index.html", "catalogue"),
        ("privacy.html", "privacy/index.html", "privacy"),
    ] {
        let context = Value::from_serialize(PageContext {
            page,
            year: 2026,
            catalog,
            home_flagships: home_flagships.clone(),
            providers: views.providers.clone(),
            flagship_models: views.flagships.clone(),
            groups: views.groups.clone(),
            legend: views.legend.clone(),
            provider_page: None,
            model_page: None,
        });
        let html = environment.get_template(template)?.render(context)?;
        write_output(&output_dir, output, html)?;
    }

    let bases = base_models(catalog);
    for provider in &views.providers {
        let groups = groups_for(
            catalog,
            &bases,
            Some(provider.id.as_str()),
            None,
        );
        let context = Value::from_serialize(PageContext {
            page: "catalogue-provider",
            year: 2026,
            catalog,
            home_flagships: Vec::new(),
            providers: Vec::new(),
            flagship_models: Vec::new(),
            groups,
            legend: Vec::new(),
            provider_page: Some(provider.clone()),
            model_page: None,
        });
        let html = environment
            .get_template("catalogue-provider.html")?
            .render(context)?;
        write_output(
            &output_dir,
            &format!("catalogue/provider/{}/index.html", provider.id),
            html,
        )?;
    }

    for vendor in &catalog.vendors {
        for model in &vendor.models {
            let groups = groups_for(
                catalog,
                &bases,
                None,
                Some(model.id.as_str()),
            );
            let context = Value::from_serialize(PageContext {
                page: "catalogue-model",
                year: 2026,
                catalog,
                home_flagships: Vec::new(),
                providers: Vec::new(),
                flagship_models: Vec::new(),
                groups,
                legend: Vec::new(),
                provider_page: None,
                model_page: Some(model_page_view(catalog, vendor, model)),
            });
            let html = environment
                .get_template("catalogue-model.html")?
                .render(context)?;
            write_output(
                &output_dir,
                &format!("catalogue/models/{}/index.html", model.id),
                html,
            )?;
        }
    }
    Ok(())
}

fn write_output(root: &Path, relative: &str, html: String) -> anyhow::Result<()> {
    let output = root.join(relative);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, html)?;
    Ok(())
}

#[derive(Clone, Serialize)]
struct PriceView {
    value: String,
    unit: String,
    text: String,
}

#[derive(Clone, Serialize)]
struct ModalityView {
    key: &'static str,
    label: &'static str,
    icon: String,
}

#[derive(Clone, Serialize)]
struct LinkView {
    label: String,
    url: String,
}

#[derive(Clone, Serialize)]
struct ProviderView {
    id: String,
    name: String,
    country: String,
    blurb: String,
    logo_url: String,
    page_url: String,
    homepage_url: Option<String>,
    types: String,
    links: Vec<LinkView>,
}

#[derive(Clone, Serialize)]
struct FlagshipView {
    id: String,
    name: String,
    company: String,
    blurb: String,
    logo_url: Option<String>,
    model_url: String,
    release: Option<String>,
    aa_score: Option<String>,
    aa_report_url: Option<String>,
    input_range: String,
    output_range: String,
    input_modalities: Vec<ModalityView>,
    output_modalities: Vec<ModalityView>,
}

#[derive(Clone, Serialize)]
struct ModelRowView {
    key: String,
    name: String,
    provider: String,
    provider_url: String,
    model_url: String,
    company: String,
    input_modalities: Vec<ModalityView>,
    output_modalities: Vec<ModalityView>,
    input_price: Option<PriceView>,
    cached_input_price: Option<PriceView>,
    output_price: Option<PriceView>,
    output_note: Option<String>,
    note: Option<String>,
    release: Option<String>,
    aa_score: Option<String>,
    aa_report_url: Option<String>,
    flagship: bool,
}

#[derive(Clone, Serialize)]
struct GroupView {
    id: String,
    name: String,
    blurb: String,
    rows: Vec<ModelRowView>,
}

#[derive(Clone, Serialize)]
struct LegendView {
    label: &'static str,
    icon: String,
}

#[derive(Clone, Serialize)]
struct ModelPageView {
    id: String,
    slug: String,
    name: String,
    company: String,
    country: String,
    blurb: String,
    logo_url: Option<String>,
    vendor_url: Option<String>,
    release: Option<String>,
    aa_score: Option<String>,
    aa_report_url: Option<String>,
    input_modalities: Vec<ModalityView>,
    output_modalities: Vec<ModalityView>,
}

fn template_environment(directory: &Path) -> anyhow::Result<Environment<'static>> {
    let mut environment = Environment::new();
    environment.set_auto_escape_callback(|name| {
        if name.ends_with(".html") {
            AutoEscape::Html
        } else {
            AutoEscape::None
        }
    });
    let mut templates = fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    templates.sort();
    for path in templates {
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .with_context(|| format!("invalid template filename: {}", path.display()))?;
        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        environment.add_template_owned(name.to_string(), source)?;
    }
    Ok(environment)
}

fn copy_dir(source: &Path, destination: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir(&source_path, &destination_path)?;
        } else {
            fs::copy(source_path, destination_path)?;
        }
    }
    Ok(())
}

fn copy_catalog_assets(catalog_dir: &Path) -> anyhow::Result<()> {
    let models_dir = catalog_dir.join("models");
    let providers_dir = catalog_dir.join("providers");
    fs::create_dir_all(&models_dir)?;
    fs::create_dir_all(&providers_dir)?;

    let catalog = mp_catalog::data();
    for logo in catalog
        .vendors
        .iter()
        .filter_map(|vendor| vendor.logo.as_deref())
        .collect::<HashSet<_>>()
    {
        let asset = mp_catalog::asset(&format!("models/{logo}"))
            .with_context(|| format!("catalog model asset '{logo}' is missing"))?;
        fs::write(models_dir.join(logo), asset.as_ref())?;
    }
    for logo in catalog
        .providers
        .iter()
        .map(|provider| provider.logo.as_str())
        .collect::<HashSet<_>>()
    {
        let asset = mp_catalog::asset(&format!("providers/{logo}"))
            .with_context(|| format!("catalog provider asset '{logo}' is missing"))?;
        fs::write(providers_dir.join(logo), asset.as_ref())?;
    }
    Ok(())
}

struct CatalogViews {
    flagships: Vec<FlagshipView>,
    providers: Vec<ProviderView>,
    groups: Vec<GroupView>,
    legend: Vec<LegendView>,
}

fn build_views(catalog: &Catalog) -> CatalogViews {
    let bases = base_models(catalog);
    let mut flagships = bases
        .values()
        .filter_map(|(vendor, model)| {
            let hosts = catalog
                .providers
                .iter()
                .flat_map(|provider| provider.models.iter().map(move |hosted| (provider, hosted)))
                .filter(|(_, hosted)| hosted.vendor_model_id == model.id && hosted.flagship)
                .collect::<Vec<_>>();
            if hosts.is_empty() {
                return None;
            }

            Some((
                model.aa_score.unwrap_or(f64::NEG_INFINITY),
                flagship_view(catalog, vendor, model, &hosts),
            ))
        })
        .collect::<Vec<_>>();
    flagships.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.1.name.cmp(&right.1.name))
    });
    let flagship_models = flagships
        .into_iter()
        .map(|(_, model)| model)
        .collect::<Vec<_>>();

    let providers = provider_views(catalog);
    let groups = groups_for(catalog, &bases, None, None);

    let legend = catalog
        .config
        .modality_order
        .iter()
        .filter_map(|modality| {
            let label = modality_label(modality)?;
            Some(LegendView {
                label,
                icon: catalog.config.icons.get(modality)?.clone(),
            })
        })
        .collect();

    CatalogViews {
        flagships: flagship_models,
        providers,
        groups,
        legend,
    }
}

fn base_models(
    catalog: &Catalog,
) -> BTreeMap<&str, (&mp_catalog::Vendor, &VendorModel)> {
    catalog
        .vendors
        .iter()
        .flat_map(|vendor| vendor.models.iter().map(move |model| (vendor, model)))
        .map(|(vendor, model)| (model.id.as_str(), (vendor, model)))
        .collect()
}

fn groups_for(
    catalog: &Catalog,
    bases: &BTreeMap<&str, (&mp_catalog::Vendor, &VendorModel)>,
    provider_id: Option<&str>,
    vendor_model_id: Option<&str>,
) -> Vec<GroupView> {
    let groups = catalog
        .config
        .sections
        .iter()
        .filter_map(|section| {
            let mut rows = model_rows(
                catalog,
                bases,
                &section.model_type,
                provider_id,
                vendor_model_id,
            );
            if rows.is_empty() {
                return None;
            }
            rows.sort_by(compare_rows);
            Some(GroupView {
                id: section_id(&section.model_type),
                name: section.model_type.clone(),
                blurb: section.blurb.clone(),
                rows,
            })
        })
        .collect();
    groups
}

fn model_rows<'a>(
    catalog: &'a Catalog,
    bases: &BTreeMap<&str, (&'a mp_catalog::Vendor, &'a VendorModel)>,
    model_type: &str,
    provider_filter: Option<&str>,
    vendor_model_filter: Option<&str>,
) -> Vec<ModelRowView> {
    catalog
        .providers
        .iter()
        .filter(|provider| {
            provider_filter.is_none_or(|id| provider.id == id)
        })
        .flat_map(|provider| {
            provider.models.iter().flat_map(move |hosted| {
                let Some((vendor, base)) = bases.get(hosted.vendor_model_id.as_str()) else {
                    return Vec::new();
                };
                if base.model_type != model_type
                    || vendor_model_filter
                        .is_some_and(|id| base.id != id)
                {
                    return Vec::new();
                }
                let input = hosted
                    .input_modalities
                    .as_deref()
                    .unwrap_or(&base.input_modalities);
                let output = hosted
                    .output_modalities
                    .as_deref()
                    .unwrap_or(&base.output_modalities);
                hosted
                    .serving_regions
                    .iter()
                    .map(|region| ModelRowView {
                        key: format!("{}/{}/{}", provider.id, region, hosted.model_id),
                        name: hosted.api_model.clone(),
                        provider: provider.name.clone(),
                        provider_url: format!("/catalogue/provider/{}/", provider.id),
                        model_url: format!("/catalogue/models/{}/", base.id),
                        company: vendor
                            .full_name
                            .clone()
                            .unwrap_or_else(|| vendor.name.clone()),
                        input_modalities: modality_views(
                            input,
                            &catalog.config.modality_order,
                            &catalog.config.icons,
                        ),
                        output_modalities: modality_views(
                            output,
                            &catalog.config.modality_order,
                            &catalog.config.icons,
                        ),
                        input_price: price_view(&hosted.pricing.input),
                        cached_input_price: hosted
                            .pricing
                            .cached_input
                            .as_ref()
                            .and_then(price_view),
                        output_price: price_view(&hosted.pricing.output),
                        output_note: hosted.pricing.output_note.clone(),
                        note: hosted.note.clone(),
                        release: base.release_date.as_deref().map(month_year),
                        aa_score: base.aa_score.map(|score| format!("{score:.1}")),
                        aa_report_url: base.aa_report_url.clone(),
                        flagship: hosted.flagship,
                    })
                    .collect()
            })
        })
        .collect()
}

fn flagship_view(
    catalog: &Catalog,
    vendor: &mp_catalog::Vendor,
    model: &VendorModel,
    hosts: &[(&Provider, &mp_catalog::ProviderModel)],
) -> FlagshipView {
    FlagshipView {
        id: model.id.clone(),
        name: model.name.clone(),
        company: vendor
            .full_name
            .clone()
            .unwrap_or_else(|| vendor.name.clone()),
        blurb: model.blurb.clone(),
        logo_url: vendor
            .logo
            .as_ref()
            .map(|logo| format!("/static/catalog/models/{logo}")),
        model_url: format!("/catalogue/models/{}/", model.id),
        release: model.release_date.as_deref().map(month_year),
        aa_score: model.aa_score.map(|score| format!("{score:.1}")),
        aa_report_url: model.aa_report_url.clone(),
        input_range: price_range(hosts, true),
        output_range: price_range(hosts, false),
        input_modalities: modality_views(
            &model.input_modalities,
            &catalog.config.modality_order,
            &catalog.config.icons,
        ),
        output_modalities: modality_views(
            &model.output_modalities,
            &catalog.config.modality_order,
            &catalog.config.icons,
        ),
    }
}

fn model_page_view(
    catalog: &Catalog,
    vendor: &mp_catalog::Vendor,
    model: &VendorModel,
) -> ModelPageView {
    ModelPageView {
        id: model.id.clone(),
        slug: model.id.clone(),
        name: model.name.clone(),
        company: vendor
            .full_name
            .clone()
            .unwrap_or_else(|| vendor.name.clone()),
        country: vendor.country.clone(),
        blurb: model.blurb.clone(),
        logo_url: vendor
            .logo
            .as_ref()
            .map(|logo| format!("/static/catalog/models/{logo}")),
        vendor_url: vendor.url.clone(),
        release: model.release_date.as_deref().map(month_year),
        aa_score: model.aa_score.map(|score| format!("{score:.1}")),
        aa_report_url: model.aa_report_url.clone(),
        input_modalities: modality_views(
            &model.input_modalities,
            &catalog.config.modality_order,
            &catalog.config.icons,
        ),
        output_modalities: modality_views(
            &model.output_modalities,
            &catalog.config.modality_order,
            &catalog.config.icons,
        ),
    }
}

fn provider_views(catalog: &Catalog) -> Vec<ProviderView> {
    let mut providers = catalog
        .providers
        .iter()
        .filter(|provider| !provider.models.is_empty())
        .map(|provider| {
            let types = catalog
                .config
                .sections
                .iter()
                .filter(|section| {
                    provider.models.iter().any(|hosted| {
                        catalog
                            .vendor_model(&hosted.vendor_model_id)
                            .is_some_and(|(_, model)| model.model_type == section.model_type)
                    })
                })
                .map(|section| section.model_type.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let homepage = provider.urls.get("homepage");
            ProviderView {
                id: provider.id.clone(),
                name: provider.name.clone(),
                country: provider.country.clone(),
                blurb: provider.blurb.clone(),
                logo_url: format!("/static/catalog/providers/{}", provider.logo),
                page_url: format!("/catalogue/provider/{}/", provider.id),
                homepage_url: homepage.map(|link| link.url.clone()),
                types,
                links: provider
                    .urls
                    .iter()
                    .filter(|(key, _)| key.as_str() != "homepage")
                    .map(|(_, link)| LinkView {
                        label: link.label.clone(),
                        url: link.url.clone(),
                    })
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    providers.sort_by(|left, right| left.name.cmp(&right.name));
    providers
}

fn modality_views(
    modalities: &[String],
    order: &[String],
    icons: &BTreeMap<String, String>,
) -> Vec<ModalityView> {
    let values = modalities
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    order
        .iter()
        .filter(|modality| values.contains(modality.as_str()))
        .filter_map(|modality| {
            Some(ModalityView {
                key: modality_label(modality)?,
                label: modality_label(modality)?,
                icon: icons.get(modality)?.clone(),
            })
        })
        .collect()
}

fn modality_label(modality: &str) -> Option<&'static str> {
    match modality {
        "audio" => Some("speech"),
        "image" => Some("image"),
        "video" => Some("video"),
        "text" => Some("text"),
        "embedding" => Some("embeddings"),
        "reranker" => Some("rerankers"),
        "typed_decision" => Some("typed decisions"),
        _ => None,
    }
}

fn price_view(price: &Price) -> Option<PriceView> {
    let value = price.value.filter(|value| *value != 0.0)?;
    let unit = price.unit.as_ref()?;
    let decimals = price.dp.unwrap_or(2) as usize;
    Some(PriceView {
        value: format!("{value:.decimals$}"),
        unit: unit.clone(),
        text: format!("{value:.decimals$}/{unit}"),
    })
}

fn price_range(hosts: &[(&Provider, &mp_catalog::ProviderModel)], input: bool) -> String {
    let mut prices = hosts
        .iter()
        .filter_map(|(_, hosted)| {
            let price = if input {
                &hosted.pricing.input
            } else {
                &hosted.pricing.output
            };
            price.unit.as_ref()?;
            price.value
        })
        .collect::<Vec<_>>();
    prices.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    match (prices.first(), prices.last()) {
        (Some(first), Some(last)) if first == last => {
            format!("€{first:.2}")
        }
        (Some(first), Some(last)) => {
            format!("€{first:.2} - €{last:.2}")
        }
        _ => "-".to_string(),
    }
}

fn compare_rows(left: &ModelRowView, right: &ModelRowView) -> std::cmp::Ordering {
    let key = |row: &ModelRowView| {
        row.output_price
            .as_ref()
            .and_then(|price| price.value.parse::<f64>().ok())
            .unwrap_or(f64::INFINITY)
    };
    key(left)
        .partial_cmp(&key(right))
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| left.name.cmp(&right.name))
}

fn output_cost(model: &FlagshipView) -> f64 {
    let prices = model
        .output_range
        .split(" - ")
        .filter_map(|value| {
            value
                .trim()
                .trim_start_matches('€')
                .parse::<f64>()
                .ok()
        })
        .collect::<Vec<_>>();
    match (prices.first(), prices.last()) {
        (Some(first), Some(last)) => (first + last) / 2.0,
        _ => f64::INFINITY,
    }
}

fn section_id(model_type: &str) -> String {
    model_type
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn month_year(value: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let (year, month) = value.split_once('-').unwrap_or((value, ""));
    month
        .parse::<usize>()
        .ok()
        .and_then(|month| MONTHS.get(month - 1))
        .map_or_else(|| value.to_string(), |month| format!("{month} {year}"))
}
