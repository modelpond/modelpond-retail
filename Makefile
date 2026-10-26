CARGO_FLAGS ?= --release --locked

.PHONY: site

site:
	cargo build $(CARGO_FLAGS)
	cp CNAME target/html/CNAME
	
dev: site
	python -m http.server -d target/html
