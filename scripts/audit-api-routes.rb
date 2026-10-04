#!/usr/bin/env ruby
# Inventory executable route registrations, independently of copied OpenAPI.
require "json"

ROOT = File.expand_path("..", __dir__)
METHODS = %w[get post put patch delete head options].freeze
RUST_ROUTERS = {
  "auth::session_routes" => ["bike-core/src/auth/controllers/auth.rs", "session_routes"],
  "auth::oauth_routes" => ["bike-core/src/auth/controllers/oauth.rs", "routes"],
  "auth::admin_routes" => ["bike-core/src/auth/controllers/admin.rs", "routes"],
  "admin::routes" => ["api/src/controllers/admin.rs", "routes"],
  "feature_flags::admin_routes" => ["bike-core/src/platform/feature_flags/admin_controller.rs", "routes"],
  "feature_flags::public_routes" => ["bike-core/src/platform/feature_flags/controller.rs", "routes"],
  "background_jobs::admin::api_routes" => ["bike-core/src/background_jobs/admin.rs", "api_routes"],
  "metrics_controller::admin_routes" => ["bike-core/src/platform/metrics_controller.rs", "admin_routes"],
  "integration_events::admin_routes" => ["api/src/controllers/integration_events.rs", "admin_routes"],
  "integration_events::routes" => ["api/src/controllers/integration_events.rs", "routes"],
  "strava::routes" => ["api/src/controllers/strava.rs", "routes"],
  "strava_gateway::routes" => ["api/src/controllers/strava_gateway.rs", "routes"]
}.freeze

def source(path)
  File.read(File.join(ROOT, path))
end

def normalized(path)
  path = "/#{path}" unless path.start_with?("/")
  path.gsub(%r{/+}, "/").gsub(/:([a-z_]+)(?=\/|$)/, '{\1}')
      .gsub(/\{(\w+):[^}]+\}/, '{\1}').sub(%r{/$}, "").then do |value|
        # Axum captures the whole filename; the heatmap controller validates
        # and strips .png before interpreting the numeric tile coordinate.
        value += ".png" if value == "/api/maps/heatmap/tiles/{z}/{x}/{y}"
        value.empty? ? "/" : value
      end
end

# Route expressions contain nested method/layer calls; a non-greedy regex
# cannot reliably find their end. Keep string literals opaque while balancing.
def closing(text, offset, opening, ending)
  depth = 0
  quoted = false
  escaped = false
  (offset...text.length).each do |index|
    char = text[index]
    if quoted
      if escaped
        escaped = false
      elsif char == "\\"
        escaped = true
      elsif char == '"'
        quoted = false
      end
    elsif char == '"'
      quoted = true
    elsif char == opening
      depth += 1
    elsif char == ending
      depth -= 1
      return index if depth.zero?
    end
  end
  abort "Unbalanced route expression at #{offset}"
end

def rust_routes(file = "api/src/controllers/mod.rs", function = "routes", prefix = "", seen = [])
  key = [file, function, prefix]
  abort "Recursive router #{key}" if seen.include?(key)
  path = "bike-rs/#{file}"
  text = source(path).gsub(/^\s*\/\/[^\n]*/, "")
  declaration = /\bfn\s+#{Regexp.escape(function)}\s*[<(]/.match(text)
  abort "Missing Rust router #{file}:#{function}" unless declaration
  start = text.index("{", declaration.end(0))
  body = text[(start + 1)...closing(text, start, "{", "}")]
  entries = []
  body.to_enum(:scan, /\.(route|nest|merge)\s*\(/).each do
    match = Regexp.last_match
    args = body[match.end(0)...closing(body, match.end(0) - 1, "(", ")")]
    if match[1] == "route"
      route_path, expression = args.match(/\A\s*"([^"]+)"\s*,\s*(.*)/m)&.captures
      abort "Unrecognized Rust route in #{file}: #{args}" unless route_path
      handlers = expression.scan(/\b(#{METHODS.join('|')})\s*\(\s*([\w:]+)/)
      abort "No methods in #{file}: #{args}" if handlers.empty?
      handlers.each do |method, handler|
        entries << {method: method.upcase, path: normalized(prefix + route_path), source: "#{path}##{handler.sub(/::$/, '')}"}
      end
    else
      # Framework-generated documentation endpoints are outside the product API.
      next if match[1] == "merge" && args.strip.start_with?("SwaggerUi::")
      nested_prefix = prefix
      if match[1] == "nest"
        mount, args = args.match(/\A\s*"([^"]+)"\s*,\s*(.*)/m)&.captures
        abort "Unrecognized mount in #{file}" unless mount
        nested_prefix += mount
      end
      call = args.strip.sub(/::<.*>\s*\(\s*\)\s*,?\s*\z/m, "()").sub(/<.*>\s*\(\s*\)\s*,?\s*\z/m, "()")
      name = call[/\A([\w:]+)\s*\(\s*\)/, 1]
      abort "Unrecognized router call in #{file}: #{args}" unless name
      target = RUST_ROUTERS[name] || (name.include?("::") ? nil : [file, name])
      abort "Unmapped Rust router #{name}" unless target
      entries.concat(rust_routes(*target, nested_prefix, seen + [key]))
    end
  end
  entries
end


def audit
  abort "Rust controller mount changed" unless source("bike-rs/api/src/lib.rs").include?("controllers::routes()")
  routes = rust_routes + rust_routes("api/src/lib.rs", "app")
  key = ->(route) { [route[:method], route[:path]] }
  contract = JSON.parse(source("contracts/openapi/openapi.json")).fetch("paths").flat_map do |path, operations|
    operations.map { |method, _| [method.upcase, normalized("/api" + path)] if METHODS.include?(method) }.compact
  end
  registered = routes.map(&key)
  result = {scope: "Rust source registrations; not runtime or behavior verification. Swagger/OpenAPI documentation mounts are excluded.",
            routes: {rust: routes.sort_by(&key)},
            rust_routes_outside_openapi: registered - contract,
            openapi_without_rust_route: contract - registered}
  output = File.join(ROOT, "docs/api-route-audit.json")
  if ARGV.include?("--write")
    File.write(output, JSON.pretty_generate(result) + "\n")
  elsif !File.file?(output) || JSON.parse(File.read(output)) != JSON.parse(JSON.generate(result))
    abort "Route audit changed; review and refresh with mise run audit:api:write"
  end
  puts "rust: #{routes.length} registrations, #{contract.length} contract operations"
  abort "OpenAPI describes unregistered Rust routes: #{contract - registered}" unless (contract - registered).empty?
end

audit if $PROGRAM_NAME == __FILE__
