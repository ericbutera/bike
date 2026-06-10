using Toybox.Lang;
using Toybox.Position;
using Toybox.System;
using Toybox.Time;

class BikeSegmentsEngine {

    const STATUS_IDLE = "idle";
    const STATUS_APPROACHING = "approaching";
    const STATUS_ACTIVE = "active";
    const STATUS_COMPLETED = "completed";
    const METERS_PER_MILE = 1609.344;
    const FEET_PER_METER = 3.28084;
    const START_TRIGGER_METERS = 20.0;
    const START_CAPTURE_METERS = 45.0;
    const START_PROGRESS_METERS = 15.0;
    const ACTIVE_ROUTE_METERS = 25.0;
    const FINISH_BUFFER_METERS = 12.0;
    const COMPLETED_OVERLAY_SECONDS = 15;

    var _sync;
    var _activeEfforts;
    var _focusIndex;
    var _activeSegment;
    var _status;
    var _message;
    var _elapsedSeconds;
    var _progressMeters;
    var _deltaSeconds;
    var _segmentStartEpoch;
    var _completedResult;
    var _completedResults;
    var _approachStartDistanceMeters;
    var _completedVisible;
    var _completedShownAt;

    function initialize(syncService) {
        _sync = syncService;
        _activeEfforts = [];
        _focusIndex = 0;
        _activeSegment = null;
        _status = STATUS_IDLE;
        _message = "Waiting for GPS";
        _elapsedSeconds = 0;
        _progressMeters = 0.0;
        _deltaSeconds = null;
        _segmentStartEpoch = null;
        _completedResult = null;
        _completedResults = [];
        _completedVisible = false;
        _completedShownAt = null;
        _approachStartDistanceMeters = {};
    }

    function tick() {
        var info = Position.getInfo();
        if (info == null || info.position == null) {
            _message = "No GPS fix";
            return;
        }

        var degrees = info.position.toDegrees();
        var lat = degrees[0].toFloat();
        var lon = degrees[1].toFloat();
        var segments = _sync.segments();

        updateActiveEfforts(lat, lon);
        var nearestApproach = updateApproachAndStarts(segments, lat, lon);
        refreshDisplayState(nearestApproach);
    }

    function updateActiveEfforts(lat, lon) {
        var nextEfforts = [];

        for (var i = 0; i < _activeEfforts.size(); i += 1) {
            var effort = _activeEfforts[i];
            if (updateEffort(effort, lat, lon)) {
                nextEfforts.add(effort);
            } else {
                // append completed efforts for the current ride and mark visible
                var comp = new SegmentCompletionResult(effort);
                _completedResults.add(comp);
                // convenience pointer to most recent
                _completedResult = comp;
                _completedVisible = true;
                _completedShownAt = Time.now().value();
            }
        }

        _activeEfforts = nextEfforts;

        if (_focusIndex >= _activeEfforts.size()) {
            _focusIndex = _activeEfforts.size() - 1;
        }
        if (_focusIndex < 0) {
            _focusIndex = 0;
        }
    }

    function updateEffort(effort, lat, lon) {
        var segment = effort.segment;
        var progress = segment.progressMeters(lat, lon);
        if (progress > effort.progressMeters) {
            effort.progressMeters = progress;
        }

        effort.elapsedSeconds = Time.now().value() - effort.startEpoch;
        effort.deltaPrSeconds = deltaAgainstTarget(effort, segment.prSeconds);
        effort.deltaKomSeconds = deltaAgainstTarget(effort, segment.komSeconds);
        effort.deltaGoalSeconds = deltaAgainstTarget(effort, preferredTargetSeconds(segment));

        if (effort.progressMeters >= (segment.distanceMeters - FINISH_BUFFER_METERS)) {
            effort.progressMeters = segment.distanceMeters;
            return false;
        }

        return true;
    }

    function deltaAgainstTarget(effort, targetSeconds) {
        if (targetSeconds == null) {
            return null;
        }

        var distanceMeters = effort.segment.distanceMeters;
        if (distanceMeters <= 0.0) {
            return null;
        }

        var expected = (effort.progressMeters / distanceMeters) * targetSeconds;
        return (effort.elapsedSeconds - expected).toNumber();
    }

    function updateApproachAndStarts(segments, lat, lon) {
        var nearestApproach = null;
        var nearestStartDistance = 1.0e12;

        for (var i = 0; i < segments.size(); i += 1) {
            var segment = segments[i];
            var segmentId = segment.id;

            if (isSegmentActive(segmentId)) {
                _approachStartDistanceMeters[segmentId] = null;
                continue;
            }

            var nearestDistance = segment.nearestDistanceMeters(lat, lon);
            if (nearestDistance > segment.approachMeters) {
                _approachStartDistanceMeters[segmentId] = null;
                continue;
            }

            var startDistance = segment.startDistanceMeters(lat, lon);
            var progressMeters = segment.progressMeters(lat, lon);
            var rememberedStartDistance = rememberApproachDistance(segmentId, startDistance);

            if (shouldBeginSegment(segment, nearestDistance, startDistance, progressMeters, rememberedStartDistance)) {
                startEffort(segment);
                _approachStartDistanceMeters[segmentId] = null;
                continue;
            }

            if (startDistance < nearestStartDistance) {
                nearestStartDistance = startDistance;
                nearestApproach = {
                    :segment => segment,
                    :start_distance => startDistance
                };
            }
        }

        return nearestApproach;
    }

    function rememberApproachDistance(segmentId, startDistance) {
        var remembered = _approachStartDistanceMeters[segmentId];
        if (remembered == null || startDistance < remembered) {
            remembered = startDistance;
            _approachStartDistanceMeters[segmentId] = remembered;
        }

        return remembered;
    }

    function shouldBeginSegment(segment, nearestDistance, startDistance, progressMeters, rememberedStartDistance) {
        if (startDistance <= START_TRIGGER_METERS) {
            return true;
        }

        if (rememberedStartDistance == null) {
            return false;
        }

        if (rememberedStartDistance > START_CAPTURE_METERS) {
            return false;
        }

        if (nearestDistance > ACTIVE_ROUTE_METERS) {
            return false;
        }

        if (progressMeters < START_PROGRESS_METERS) {
            return false;
        }

        if (progressMeters >= (segment.distanceMeters - FINISH_BUFFER_METERS)) {
            return false;
        }

        return startDistance > (rememberedStartDistance + 3.0);
    }

    function startEffort(segment) {
        if (isSegmentActive(segment.id)) {
            return;
        }

        var effort = new SegmentEffortState(segment, Time.now().value());
        _activeEfforts.add(effort);
        _focusIndex = _activeEfforts.size() - 1;
        _completedResult = null;
    }

    function isSegmentActive(segmentId) {
        for (var i = 0; i < _activeEfforts.size(); i += 1) {
            if (_activeEfforts[i].segment.id == segmentId) {
                return true;
            }
        }

        return false;
    }

    function refreshDisplayState(nearestApproach) {
        // If a completed result is visible, show it briefly before returning
        // to the normal home-state rendering.
        if (_completedVisible && _completedShownAt != null) {
            var now = Time.now().value();
            var age = now - _completedShownAt;
            if (age < COMPLETED_OVERLAY_SECONDS) {
                // show the most recent completed result
                var recent = _completedResult;
                if (recent == null && _completedResults != null && _completedResults.size() > 0) {
                    recent = _completedResults[_completedResults.size() - 1];
                }

                if (recent != null) {
                    _status = STATUS_COMPLETED;
                    _activeSegment = recent.segment;
                    _elapsedSeconds = recent.elapsedSeconds;
                    _progressMeters = recent.segment.distanceMeters;
                    _deltaSeconds = recent.deltaPrSeconds;
                    _segmentStartEpoch = null;
                    _message = "Completed " + recent.segment.title + " in " + formatSeconds(recent.elapsedSeconds);
                    return;
                }
            } else {
                // expired, hide completed overlay and continue
                _completedVisible = false;
                _completedShownAt = null;
                _completedResult = null;
            }
        }

        // No visible completed overlay; normal state selection
        var effort = focusedEffort();
        if (effort != null) {
            _status = STATUS_ACTIVE;
            _activeSegment = effort.segment;
            _elapsedSeconds = effort.elapsedSeconds;
            _progressMeters = effort.progressMeters;
            _deltaSeconds = effort.deltaGoalSeconds;
            _segmentStartEpoch = effort.startEpoch;
            _message = effort.segment.title + " " + formatDistance(effort.progressMeters) + " / " + formatDistance(effort.segment.distanceMeters);
            return;
        }

        if (nearestApproach != null) {
            _status = STATUS_APPROACHING;
            _activeSegment = nearestApproach[:segment];
            _elapsedSeconds = 0;
            _progressMeters = 0.0;
            _deltaSeconds = null;
            _segmentStartEpoch = null;
            _message = "Approaching " + _activeSegment.title + " (" + formatDistance(nearestApproach[:start_distance]) + ")";
            return;
        }

        _status = STATUS_IDLE;
        _activeSegment = null;
        _elapsedSeconds = 0;
        _progressMeters = 0.0;
        _deltaSeconds = null;
        _segmentStartEpoch = null;
        _message = "No nearby segments";
    }

    function preferredTargetSeconds(segment) {
        if (segment.prSeconds != null) {
            return segment.prSeconds;
        }

        return segment.goalSeconds;
    }

    function status() {
        return _status;
    }

    function message() {
        return _message;
    }

    function elapsedSeconds() {
        return _elapsedSeconds;
    }

    function deltaSeconds() {
        return _deltaSeconds;
    }

    function progressMeters() {
        return _progressMeters;
    }

    function activeEffortCount() {
        return _activeEfforts.size();
    }

    function focusPosition() {
        if (_activeEfforts.size() == 0) {
            return 0;
        }

        return _focusIndex + 1;
    }

    function focusedEffort() {
        if (_activeEfforts.size() == 0) {
            return null;
        }

        if (_focusIndex >= _activeEfforts.size()) {
            _focusIndex = _activeEfforts.size() - 1;
        }
        if (_focusIndex < 0) {
            _focusIndex = 0;
        }

        return _activeEfforts[_focusIndex];
    }

    function completedResult() {
        // Only expose the most-recent completed result while it's visible
        if (!_completedVisible) {
            return null;
        }

        if (_completedResults == null || _completedResults.size() == 0) {
            return null;
        }

        return _completedResults[_completedResults.size() - 1];
    }

    function dismissCompletedResult() {
        // Hide the completed result (keep it in per-ride history)
        _completedVisible = false;
        _completedShownAt = null;
        _completedResult = null;
    }

    function completedResults() {
        return _completedResults;
    }

    function focusNextEffort() {
        if (_activeEfforts.size() <= 1) {
            return;
        }

        _focusIndex = (_focusIndex + 1) % _activeEfforts.size();
    }

    function focusPreviousEffort() {
        if (_activeEfforts.size() <= 1) {
            return;
        }

        _focusIndex -= 1;
        if (_focusIndex < 0) {
            _focusIndex = _activeEfforts.size() - 1;
        }
    }

    function formatSeconds(value) {
        var min = value / 60;
        var sec = value % 60;
        return min.format("%d") + ":" + sec.format("%02d");
    }

    function formatDistance(value) {
        var meters = value.toFloat();
        var miles = meters / METERS_PER_MILE;

        if (miles >= 0.2) {
            if (miles >= 100.0) {
                return miles.format("%.0f") + " mi";
            }

            if (miles >= 10.0) {
                return miles.format("%.1f") + " mi";
            }

            return miles.format("%.2f") + " mi";
        }

        return (meters * FEET_PER_METER).format("%.0f") + " ft";
    }
}

class SegmentEffortState {
    var segment;
    var startEpoch;
    var elapsedSeconds;
    var progressMeters;
    var deltaGoalSeconds;
    var deltaPrSeconds;
    var deltaKomSeconds;

    function initialize(segmentValue, startEpochValue) {
        segment = segmentValue;
        startEpoch = startEpochValue;
        elapsedSeconds = 0;
        progressMeters = 0.0;
        deltaGoalSeconds = null;
        deltaPrSeconds = null;
        deltaKomSeconds = null;
    }
}

class SegmentCompletionResult {
    var segment;
    var elapsedSeconds;
    var deltaPrSeconds;
    var deltaKomSeconds;
    var deltaTop10Seconds;
    var overallRank;
    var overallRankExact;
    var isPr;
    var isKom;
    var isTop10;

    function initialize(effort) {
        segment = effort.segment;
        elapsedSeconds = effort.elapsedSeconds;
        deltaPrSeconds = resultDelta(elapsedSeconds, segment.prSeconds);
        deltaKomSeconds = resultDelta(elapsedSeconds, segment.komSeconds);
        deltaTop10Seconds = resultDelta(elapsedSeconds, segment.top10Seconds);
        overallRank = calculateOverallRank(elapsedSeconds, segment);
        overallRankExact = hasLeaderboard(segment);
        isPr = (deltaPrSeconds != null) && (deltaPrSeconds <= 0);
        isKom = (deltaKomSeconds != null) && (deltaKomSeconds <= 0);
        isTop10 = calculateTop10();
    }

    function resultDelta(actualSeconds, targetSeconds) {
        if (targetSeconds == null) {
            return null;
        }

        return actualSeconds - targetSeconds;
    }

    function calculateTop10() {
        if (overallRank != null) {
            return overallRank <= 10;
        }

        return (deltaTop10Seconds != null) && (deltaTop10Seconds <= 0);
    }

    function hasLeaderboard(segmentValue) {
        return segmentValue.leaderboardSeconds != null && segmentValue.leaderboardSeconds.size() > 0;
    }

    function calculateOverallRank(actualSeconds, segmentValue) {
        if (hasLeaderboard(segmentValue)) {
            var rank = 1;
            var leaderboard = segmentValue.leaderboardSeconds;
            for (var i = 0; i < leaderboard.size(); i += 1) {
                var leaderboardTime = leaderboard[i];
                if (leaderboardTime != null && actualSeconds > leaderboardTime) {
                    rank += 1;
                }
            }

            if (rank > leaderboard.size() && leaderboard.size() >= 10) {
                return 11;
            }

            return rank;
        }

        if (deltaKomSeconds != null && deltaKomSeconds <= 0) {
            return 1;
        }

        if (deltaTop10Seconds != null) {
            if (deltaTop10Seconds <= 0) {
                return 10;
            }

            return 11;
        }

        return null;
    }
}
