using Toybox.Lang;
using Toybox.Math;

class SegmentModel {
    var id;
    var title;
    var distanceMeters;
    var approachMeters;
    var routePoints;
    var goalSeconds;
    var prSeconds;
    var komSeconds;
    var top10Seconds;
    var leaderboardSeconds;
    var lastAttemptSeconds;
    var lastAttemptAt;

    function initialize(payload) {
        id = payloadValue(payload, :id, "id");
        title = payloadValue(payload, :title, "title");
        distanceMeters = floatValue(payloadValue(payload, :distance_meters, "distance_meters"));
        approachMeters = floatValue(payloadValue(payload, :approach_meters, "approach_meters"));
        routePoints = payloadValue(payload, :route_points, "route_points");
        if (routePoints == null || !(routePoints instanceof Lang.Array)) {
            routePoints = [];
        }

        goalSeconds = payloadValue(payload, :goal_seconds, "goal_seconds");
        prSeconds = payloadValue(payload, :pr_seconds, "pr_seconds");
        komSeconds = payloadValue(payload, :kom_seconds, "kom_seconds");
        top10Seconds = payloadValue(payload, :top10_seconds, "top10_seconds");
        leaderboardSeconds = arrayValue(payloadValue(payload, :leaderboard_seconds, "leaderboard_seconds"));
        if (leaderboardSeconds.size() == 0) {
            leaderboardSeconds = arrayValue(payloadValue(payload, :top_10_seconds, "top_10_seconds"));
        }
        lastAttemptSeconds = payloadValue(payload, :last_attempt_seconds, "last_attempt_seconds");
        lastAttemptAt = payloadValue(payload, :last_attempt_at, "last_attempt_at");
    }

    function nearestDistanceMeters(lat, lon) {
        var best = 1.0e12;

        for (var i = 0; i < routePoints.size(); i += 1) {
            var point = routePoints[i];
            var d = haversineMeters(
                lat,
                lon,
                floatValue(payloadValue(point, :lat, "lat")),
                floatValue(payloadValue(point, :lon, "lon"))
            );
            if (d < best) {
                best = d;
            }
        }

        return best;
    }

    function progressMeters(lat, lon) {
        var bestDistance = 1.0e12;
        var bestProgress = 0.0;

        for (var i = 0; i < routePoints.size(); i += 1) {
            var point = routePoints[i];
            var d = haversineMeters(
                lat,
                lon,
                floatValue(payloadValue(point, :lat, "lat")),
                floatValue(payloadValue(point, :lon, "lon"))
            );
            if (d < bestDistance) {
                bestDistance = d;
                bestProgress = floatValue(payloadValue(point, :distance_meters, "distance_meters"));
            }
        }

        return bestProgress;
    }

    function startDistanceMeters(lat, lon) {
        if (routePoints.size() == 0) {
            return 1.0e12;
        }

        var start = routePoints[0];
        return haversineMeters(
            lat,
            lon,
            floatValue(payloadValue(start, :lat, "lat")),
            floatValue(payloadValue(start, :lon, "lon"))
        );
    }

    function pointAtDistanceMeters(distanceMetersValue) {
        if (routePoints.size() == 0) {
            return null;
        }

        if (distanceMetersValue <= 0.0 || routePoints.size() == 1) {
            return routePoints[0];
        }

        var previous = routePoints[0];
        var previousDistance = floatValue(payloadValue(previous, :distance_meters, "distance_meters"));

        for (var i = 1; i < routePoints.size(); i += 1) {
            var current = routePoints[i];
            var currentDistance = floatValue(payloadValue(current, :distance_meters, "distance_meters"));

            if (distanceMetersValue <= currentDistance) {
                var span = currentDistance - previousDistance;
                if (span <= 0.0) {
                    return current;
                }

                var ratio = (distanceMetersValue - previousDistance) / span;
                return {
                    :lat => interpolateValue(previous, current, ratio, :lat, "lat"),
                    :lon => interpolateValue(previous, current, ratio, :lon, "lon")
                };
            }

            previous = current;
            previousDistance = currentDistance;
        }

        return routePoints[routePoints.size() - 1];
    }

    function payloadValue(payload, symbolKey, stringKey) {
        if (payload == null || !(payload instanceof Lang.Dictionary)) {
            return null;
        }

        var value = payload[symbolKey];
        if (value != null) {
            return value;
        }

        return payload[stringKey];
    }

    function floatValue(value) {
        if (value == null) {
            return 0.0;
        }

        return value.toFloat();
    }

    function arrayValue(value) {
        if (value != null && (value instanceof Lang.Array)) {
            return value;
        }

        return [];
    }

    function interpolateValue(previous, current, ratio, symbolKey, stringKey) {
        var startValue = floatValue(payloadValue(previous, symbolKey, stringKey));
        var endValue = floatValue(payloadValue(current, symbolKey, stringKey));
        return startValue + ((endValue - startValue) * ratio);
    }

    function haversineMeters(lat1, lon1, lat2, lon2) {
        var degToRad = Math.PI / 180.0;
        var dLat = (lat2 - lat1) * degToRad;
        var dLon = (lon2 - lon1) * degToRad;
        var a = Math.sin(dLat / 2.0) * Math.sin(dLat / 2.0)
            + Math.cos(lat1 * degToRad) * Math.cos(lat2 * degToRad)
            * Math.sin(dLon / 2.0) * Math.sin(dLon / 2.0);
        var c = 2.0 * Math.atan2(Math.sqrt(a), Math.sqrt(1.0 - a));
        return 6371000.0 * c;
    }
}
