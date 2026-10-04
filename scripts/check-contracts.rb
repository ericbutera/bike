#!/usr/bin/env ruby

require "digest"
require "json"

root = File.expand_path("..", __dir__)
roots = {
  "rust" => ENV.fetch("BIKE_RUST_DIR", File.join(root, "bike-rs")),
}
ui_root = ENV.fetch("BIKE_UI_DIR", File.join(root, "bike-ui"))
fixture_root = File.join(root, "bike-rs/api/tests/fixtures/platform")
failures = []

def compare_contract(roots, canonical, filename, failures)
  reference_path = File.join(canonical, filename)
  reference = File.binread(reference_path)
  reference_digest = Digest::SHA256.hexdigest(reference)

  roots.each do |name, backend_root|
    candidate_path = File.join(backend_root, "docs/openapi/#{filename}")
    candidate = File.file?(candidate_path) ? File.binread(candidate_path) : nil
    digest = candidate ? Digest::SHA256.hexdigest(candidate) : "missing"
    status = candidate == reference ? "PASS" : "FAIL"
    puts "#{status} #{name} docs/openapi/#{filename} sha256=#{digest} canonical=#{reference_digest}"
    failures << "#{name}: docs/openapi/#{filename}" unless candidate == reference
  end
end

roots.each do |name, backend_root|
  unless File.directory?(backend_root)
    puts "FAIL missing backend directory: #{name} #{backend_root}"
    failures << "missing backend: #{name}"
  end
end

canonical = File.join(root, "contracts/openapi")
%w[openapi.json openapi.yaml].each do |filename|
  compare_contract(roots, canonical, filename, failures)
  failures << "missing canonical contract: #{filename}" unless File.file?(File.join(canonical, filename))
end

asset_inventory = JSON.parse(File.binread(File.join(root, "docs/shared-assets.json")))
asset_inventory.fetch("assets").each do |asset|
  next if asset.fetch("check") == "http-contract"

  source_path = File.join(root, asset.fetch("canonical"))
  source = File.file?(source_path) ? File.binread(source_path) : nil
  asset.fetch("mirrors").each do |mirror|
    mirror_path = File.join(root, mirror)
    candidate = File.file?(mirror_path) ? File.binread(mirror_path) : nil
    matches = source && candidate == source
    puts "#{matches ? "PASS" : "FAIL"} shared asset #{mirror} owner=#{asset.fetch("canonical")}"
    failures << "shared asset: #{mirror}" unless matches
  end
end

openapi = JSON.parse(File.binread(File.join(canonical, "openapi.json")))
operations = openapi.fetch("paths").flat_map do |path, methods|
  methods.each_with_object([]) do |(method, operation), entries|
    entries << [method, path, operation["operationId"]] if operation.is_a?(Hash) && operation["operationId"]
  end
end

route_matrix_path = File.join(fixture_root, "routes.json")
route_matrix = JSON.parse(File.binread(route_matrix_path))
route_matrix.each do |route|
  next if route["kind"] == "validation"

  operation_id = route.fetch("operationId")
  expected = operations.find { |method, _, candidate| method == "get" && candidate == operation_id }
  unless expected
    puts "FAIL API fixture route inventory operation #{operation_id} is absent from OpenAPI"
    failures << "API fixture route inventory operation: #{operation_id}"
    next
  end

  contract_path = route.fetch("contractPath", route.fetch("path")).split("?", 2).first
  contract_path = contract_path.delete_prefix("/api").gsub(/\{\{\w+\}\}/, "{id}")
  unless contract_path == expected[1]
    puts "FAIL API fixture route inventory path #{operation_id}: #{contract_path} != #{expected[1]}"
    failures << "API fixture route inventory path: #{operation_id}"
  end
end

legacy_auth_operations = %w[
  forgot_password
  login
  register
  resend_confirmation
  reset_password
  verify_email
]
unexpected_legacy_routes = route_matrix.map { |route| route["operationId"] } & legacy_auth_operations
unless unexpected_legacy_routes.empty?
  puts "FAIL API fixture route inventory includes legacy auth: #{unexpected_legacy_routes.join(", ")}"
  failures << "API fixture route inventory: legacy auth"
end

shared_ui_files = JSON.parse(File.binread(File.join(ui_root, "tests/e2e/required-ui-files.json")))
shared_ui_files.each do |relative_path|
  exists = File.file?(File.join(ui_root, relative_path))
  puts "#{exists ? "PASS" : "FAIL"} shared UI #{relative_path}"
  failures << "missing shared UI file: #{relative_path}" unless exists
end

package = JSON.parse(File.binread(File.join(ui_root, "package.json")))
package_dependencies = %w[dependencies devDependencies optionalDependencies peerDependencies].flat_map do |section|
  package.fetch(section, {}).keys
end
if package_dependencies.any? { |dependency| dependency.downcase.include?("kaleido") }
  puts "FAIL shared UI depends on Kaleido"
  failures << "shared UI depends on Kaleido"
else
  puts "PASS shared UI has no Kaleido package dependency"
end

roots.each do |name, backend_root|
  %w[ui-next bike-ui ui frontend].each do |directory|
    if File.directory?(File.join(backend_root, directory))
      puts "FAIL #{name} contains an embedded #{directory} tree"
      failures << "#{name}: embedded #{directory}"
    end
  end
end

rust_root = roots.fetch("rust")
rust_platform_files = Dir.glob(File.join(rust_root, "**", "{Cargo.toml,*.rs}"))
rust_platform_files.reject! { |path| path.include?("/target/") }
rust_platform_files << File.join(rust_root, "Cargo.lock")
rust_platform_files.each do |path|
  source = File.read(path)
  dependency = File.basename(path).start_with?("Cargo.") && source.match?(/\bkaleido\b/i)
  import = path.end_with?(".rs") && source.match?(/\bkaleido\s*::/)
  if dependency || import
    puts "FAIL Rust Kaleido dependency/import: #{path.delete_prefix(rust_root + "/")}"
    failures << "Rust Kaleido dependency/import: #{path}"
  end
end
puts "PASS Rust has no Kaleido dependency/import" unless failures.any? { |failure| failure.start_with?("Rust Kaleido") }

if failures.empty?
  puts "Static contract and shared UI checks passed"
  exit 0
end

warn "Static contract and shared UI checks failed: #{failures.join(", ")}"
exit 1
