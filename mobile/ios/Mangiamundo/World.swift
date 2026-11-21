import Foundation

/// A country outline from `world.json`.
struct WorldCountry {
    let code: String
    let name: String
    /// Rings as flat `[lng, lat, lng, lat, ...]` arrays. Holes are rings too: fill even-odd.
    let rings: [[Double]]
    let minLng: Double
    let maxLng: Double
    let minLat: Double
    let maxLat: Double

    init(code: String, name: String, rings: [[Double]]) {
        self.code = code
        self.name = name
        self.rings = rings
        var west = Double.greatestFiniteMagnitude, east = -Double.greatestFiniteMagnitude
        var south = Double.greatestFiniteMagnitude, north = -Double.greatestFiniteMagnitude
        for ring in rings {
            for i in stride(from: 0, to: ring.count - 1, by: 2) {
                west = min(west, ring[i]); east = max(east, ring[i])
                south = min(south, ring[i + 1]); north = max(north, ring[i + 1])
            }
        }
        minLng = west; maxLng = east; minLat = south; maxLat = north
    }

    /// Even-odd point-in-polygon test.
    func contains(lng: Double, lat: Double) -> Bool {
        guard lat >= minLat, lat <= maxLat, lng >= minLng, lng <= maxLng else { return false }
        var inside = false
        for ring in rings where ring.count >= 6 {
            var j = ring.count - 2
            for i in stride(from: 0, to: ring.count - 1, by: 2) {
                let xi = ring[i], yi = ring[i + 1]
                let xj = ring[j], yj = ring[j + 1]
                if (yi > lat) != (yj > lat), lng < (xj - xi) * (lat - yi) / (yj - yi) + xi {
                    inside.toggle()
                }
                j = i
            }
        }
        return inside
    }
}

struct World {
    let countries: [WorldCountry]
    /// The countries each one borders: outlines that share a vertex.
    let neighbours: [String: Set<String>]

    init(countries: [WorldCountry]) {
        self.countries = countries
        var at: [SIMD2<Double>: Set<String>] = [:]
        for country in countries {
            for ring in country.rings {
                for i in stride(from: 0, to: ring.count - 1, by: 2) {
                    at[SIMD2(ring[i], ring[i + 1]), default: []].insert(country.code)
                }
            }
        }
        var neighbours: [String: Set<String>] = [:]
        for codes in at.values where codes.count > 1 {
            for code in codes { neighbours[code, default: []].formUnion(codes.subtracting([code])) }
        }
        self.neighbours = neighbours
    }

    /// Groups `codes` into runs of bordering countries: each code maps to its run's first code.
    func regions(_ codes: Set<String>) -> [String: String] {
        var region: [String: String] = [:]
        for start in codes where region[start] == nil {
            var queue = [start]
            region[start] = start
            while let code = queue.popLast() {
                for next in neighbours[code] ?? [] where codes.contains(next) && region[next] == nil {
                    region[next] = start
                    queue.append(next)
                }
            }
        }
        return region
    }

    /// The country at a position, or nil over the sea.
    func country(lng: Double, lat: Double) -> WorldCountry? {
        // Some rings run past the antimeridian (lng > 180 or < -180), so try the wrapped copies.
        for shift in [0.0, 360.0, -360.0] {
            if let found = countries.first(where: { $0.contains(lng: lng + shift, lat: lat) }) {
                return found
            }
        }
        return nil
    }

    static func parse(_ data: Data) throws -> World {
        struct Raw: Decodable {
            let code: String
            let name: String
            let rings: [[Double]]
        }
        let raw = try JSONDecoder().decode([Raw].self, from: data)
        return World(countries: raw.map { WorldCountry(code: $0.code, name: $0.name, rings: $0.rings) })
    }

    static func bundled() -> World? {
        guard let url = Bundle.main.url(forResource: "world", withExtension: "json"),
              let data = try? Data(contentsOf: url) else { return nil }
        return try? parse(data)
    }
}
