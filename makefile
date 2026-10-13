.PHONY: compile clean prepare test buildtests

compile:
	cargo build --release --locked
clean:
	cargo clean
prepare:
	cargo fetch --locked
test:
	cargo test --locked
buildtests:
	cd tests && lua buildtests.lua
