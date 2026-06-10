using Toybox.Graphics;
using Toybox.Timer;
using Toybox.WatchUi;

class BikeSegmentsView extends WatchUi.View {

    const METERS_PER_MILE = 1609.344;
    const FEET_PER_METER = 3.28084;

    var _sync;
    var _engine;
    var _timer;
    var _tickCount;
    var _pendingMenuAction;

    function initialize() {
        View.initialize();
        _sync = new SegmentSyncService();
        _engine = new BikeSegmentsEngine(_sync);
        _timer = new Timer.Timer();
        _tickCount = 0;
        _pendingMenuAction = null;
    }

    function queueMenuAction(action) {
        _pendingMenuAction = action;
    }

    function beginPairing() {
        _sync.beginLink();
        WatchUi.requestUpdate();
    }

    function resetPairing() {
        _sync.requestReset();
        WatchUi.requestUpdate();
    }

    function browseSegments() {
        var browserView = new $.SegmentBrowserView(_sync);

        WatchUi.pushView(
            browserView,
            new $.SegmentBrowserDelegate(browserView),
            WatchUi.SLIDE_IMMEDIATE
        );
    }

    function showRideSummary() {
        var summary = new $.RideSummaryView(_engine);
        WatchUi.pushView(summary, new $.RideSummaryDelegate(summary), WatchUi.SLIDE_LEFT);
    }

    function hasSyncedSegments() {
        return _sync.segments().size() > 0;
    }

    function browserMenuLabel() {
        var count = _sync.segments().size();
        if (count <= 0) {
            return "Segments";
        }

        return "Segments (" + count.format("%d") + ")";
    }

    function showNextActive() {
        if (engineStatusEquals("active")) {
            _engine.focusNextEffort();
            WatchUi.requestUpdate();
            return true;
        }

        return false;
    }

    function showPreviousActive() {
        if (engineStatusEquals("active")) {
            _engine.focusPreviousEffort();
            WatchUi.requestUpdate();
            return true;
        }

        return false;
    }

    function handlePrimarySelect() {
        if (engineStatusEquals("completed") && _engine.completedResult() != null) {
            _engine.dismissCompletedResult();
            _engine.tick();
            WatchUi.requestUpdate();
            return true;
        }

        return false;
    }

    function onShow() {
        runPendingMenuAction();
        _sync.syncNow();

        _timer.start(
            method(:onTick),
            1000,
            true
        );
    }

    function onHide() {
        _timer.stop();
    }

    function onTick() {
        runPendingMenuAction();
        _tickCount += 1;
        if ((_tickCount % 5) == 0) {
            _sync.syncNow();
        }

        _engine.tick();
        WatchUi.requestUpdate();
    }

    function onUpdate(dc) {
        dc.clear();
        dc.setColor(Graphics.COLOR_WHITE, Graphics.COLOR_BLACK);

        if (engineStatusEquals("active")) {
            drawActiveScreen(dc);
            return;
        }

        if (engineStatusEquals("completed")) {
            drawCompletedScreen(dc);
            return;
        }

        drawHomeScreen(dc);
    }

    function drawHomeScreen(dc) {
        var height = dc.getHeight();
        var left = 12;
        var y = 16;
        var debugLines = _sync.debugLines();
        var debugStart = height - 38;

        dc.drawText(left, y, Graphics.FONT_LARGE, "Bike Segments", Graphics.TEXT_JUSTIFY_LEFT);
        y += 30;
        drawInfoLine(dc, left, y, "Sync", _sync.lastSyncStatus(), 34);
        y += 18;
        drawInfoLine(dc, left, y, "API", _sync.apiEnvironmentLabel(), 28);
        y += 18;
        drawInfoLine(dc, left, y, "Segments", _sync.segments().size().format("%d"), 8);
        y += 20;

        var linkCode = _sync.linkCode();
        if (linkCode != null) {
            drawInfoLine(dc, left, y, "Code", linkCode, 16);
            y += 18;
            drawInfoLine(dc, left, y, "Open", _sync.verificationLabel(), 30);
            y += 20;
        } else {
            drawInfoLine(dc, left, y, "State", _engine.status(), 18);
            y += 18;
            dc.drawText(
                left,
                y,
                Graphics.FONT_SMALL,
                trimText(_engine.message(), 34),
                Graphics.TEXT_JUSTIFY_LEFT
            );
            y += 24;
        }

        var lastErr = _sync.lastError();
        if (lastErr != null) {
            dc.drawText(
                left,
                y,
                Graphics.FONT_TINY,
                trimText("Err: " + lastErr, 38),
                Graphics.TEXT_JUSTIFY_LEFT
            );
            y += 18;
        }

        var delta = _engine.deltaSeconds();
        if (delta != null) {
            dc.drawText(
                left,
                y,
                Graphics.FONT_SMALL,
                "Delta " + formatSignedSeconds(delta),
                Graphics.TEXT_JUSTIFY_LEFT
            );
            y += 20;
        }

        dc.drawText(left, y, Graphics.FONT_SMALL, "Elapsed " + _engine.elapsedSeconds().format("%d") + "s", Graphics.TEXT_JUSTIFY_LEFT);

        for (var i = 0; i < debugLines.size() && i < 2; i += 1) {
            dc.drawText(
                left,
                debugStart + (i * 16),
                Graphics.FONT_TINY,
                trimText(debugLines[i], 38),
                Graphics.TEXT_JUSTIFY_LEFT
            );
        }
    }

    function drawActiveScreen(dc) {
        var effort = _engine.focusedEffort();
        if (effort == null) {
            drawHomeScreen(dc);
            return;
        }

        var segment = effort.segment;
        var left = 12;
        var width = dc.getWidth();
        var height = dc.getHeight();
        var y = 10;
        var title = trimText(segment.title, 22);
        var activeCount = _engine.activeEffortCount();

        if (activeCount > 1) {
            title += " " + _engine.focusPosition().format("%d") + "/" + activeCount.format("%d");
        }

        dc.drawText(left, y, Graphics.FONT_SMALL, title, Graphics.TEXT_JUSTIFY_LEFT);
        y += 18;
        drawInfoLine(dc, left, y, "Elapsed", formatSeconds(effort.elapsedSeconds), 16);
        y += 14;
        drawInfoLine(dc, left, y, "PR", formatSignedSeconds(effort.deltaPrSeconds), 14);
        y += 14;
        drawInfoLine(dc, left, y, "KOM", formatSignedSeconds(effort.deltaKomSeconds), 14);
        y += 14;
        dc.drawText(
            left,
            y,
            Graphics.FONT_TINY,
            trimText(formatDistance(effort.progressMeters) + " / " + formatDistance(segment.distanceMeters), 28),
            Graphics.TEXT_JUSTIFY_LEFT
        );
        y += 16;

        if (activeCount > 1) {
            dc.drawText(left, y, Graphics.FONT_TINY, "Up/Down to switch active segment", Graphics.TEXT_JUSTIFY_LEFT);
            y += 14;
        }

        drawActiveRoutePreview(dc, effort, left, y, width - (left * 2), height - y - 10);
    }

    function drawCompletedScreen(dc) {
        var result = _engine.completedResult();
        if (result == null) {
            drawHomeScreen(dc);
            return;
        }

        var left = 12;
        var height = dc.getHeight();
        var y = 12;
        var headline = "Segment Complete";

        if (result.isKom) {
            headline = "KOM RIDE";
        } else if (result.isPr) {
            headline = "NEW PR";
        } else if (result.isTop10) {
            headline = "Top 10";
        }

        dc.drawText(left, y, Graphics.FONT_SMALL, headline, Graphics.TEXT_JUSTIFY_LEFT);
        y += 18;
        dc.drawText(left, y, Graphics.FONT_TINY, trimText(result.segment.title, 34), Graphics.TEXT_JUSTIFY_LEFT);
        y += 18;
        drawInfoLine(dc, left, y, "Time", formatSeconds(result.elapsedSeconds), 16);
        y += 14;
        drawInfoLine(dc, left, y, "Rank", rankLabel(result), 14);
        y += 14;
        drawInfoLine(dc, left, y, "vs PR", formatSignedSeconds(result.deltaPrSeconds), 14);
        y += 14;
        drawInfoLine(dc, left, y, "vs KOM", formatSignedSeconds(result.deltaKomSeconds), 14);
        y += 18;

        if (result.isTop10) {
            drawInfoLine(dc, left, y, "Top 10", "Yes", 12);
            y += 14;
        }

        if (result.isPr) {
            dc.drawText(left, y, Graphics.FONT_SMALL, "Personal best achieved", Graphics.TEXT_JUSTIFY_LEFT);
            y += 18;
        }

        dc.drawText(left, height - 18, Graphics.FONT_TINY, "Select to dismiss", Graphics.TEXT_JUSTIFY_LEFT);
    }

    function drawActiveRoutePreview(dc, effort, left, top, width, height) {
        drawBox(dc, left, top, width, height);

        var segment = effort.segment;
        var routePoints = segment.routePoints;
        if (routePoints == null || routePoints.size() < 2) {
            dc.drawText(left + 10, top + 18, Graphics.FONT_TINY, "No route preview", Graphics.TEXT_JUSTIFY_LEFT);
            return;
        }

        var minLat = pointFloat(routePoints[0], :lat, "lat");
        var maxLat = minLat;
        var minLon = pointFloat(routePoints[0], :lon, "lon");
        var maxLon = minLon;

        for (var i = 1; i < routePoints.size(); i += 1) {
            var lat = pointFloat(routePoints[i], :lat, "lat");
            var lon = pointFloat(routePoints[i], :lon, "lon");

            if (lat < minLat) {
                minLat = lat;
            }
            if (lat > maxLat) {
                maxLat = lat;
            }
            if (lon < minLon) {
                minLon = lon;
            }
            if (lon > maxLon) {
                maxLon = lon;
            }
        }

        var latSpan = maxLat - minLat;
        var lonSpan = maxLon - minLon;
        if (latSpan < 0.000001) {
            latSpan = 0.000001;
        }
        if (lonSpan < 0.000001) {
            lonSpan = 0.000001;
        }

        var padding = 10;
        var plotWidth = width - (padding * 2);
        var plotHeight = height - (padding * 2);
        var scaleX = plotWidth / lonSpan;
        var scaleY = plotHeight / latSpan;
        var scale = scaleX;
        if (scaleY < scale) {
            scale = scaleY;
        }

        var offsetX = left + padding + ((plotWidth - (lonSpan * scale)) / 2);
        var offsetY = top + padding + ((plotHeight - (latSpan * scale)) / 2);

        for (var pointIndex = 1; pointIndex < routePoints.size(); pointIndex += 1) {
            var previous = routePoints[pointIndex - 1];
            var current = routePoints[pointIndex];
            dc.drawLine(
                projectPreviewX(previous, minLon, scale, offsetX).toNumber(),
                projectPreviewY(previous, maxLat, scale, offsetY).toNumber(),
                projectPreviewX(current, minLon, scale, offsetX).toNumber(),
                projectPreviewY(current, maxLat, scale, offsetY).toNumber()
            );
        }

        drawPreviewMarker(dc, routePoints[0], minLon, maxLat, scale, offsetX, offsetY, "S");
        drawPreviewMarker(dc, routePoints[routePoints.size() - 1], minLon, maxLat, scale, offsetX, offsetY, "F");
        drawDistanceMarker(dc, segment, effort.progressMeters, minLon, maxLat, scale, offsetX, offsetY, "ME");

        var prProgress = ghostProgressMeters(effort.elapsedSeconds, segment.prSeconds, segment.distanceMeters);
        if (prProgress != null) {
            drawDistanceMarker(dc, segment, prProgress, minLon, maxLat, scale, offsetX, offsetY, "PR");
        }

        var komProgress = ghostProgressMeters(effort.elapsedSeconds, segment.komSeconds, segment.distanceMeters);
        if (komProgress != null) {
            drawDistanceMarker(dc, segment, komProgress, minLon, maxLat, scale, offsetX, offsetY, "K");
        }
    }

    function ghostProgressMeters(elapsedSeconds, targetSeconds, distanceMeters) {
        if (targetSeconds == null || targetSeconds <= 0 || distanceMeters <= 0.0) {
            return null;
        }

        var progress = (elapsedSeconds.toFloat() / targetSeconds.toFloat()) * distanceMeters;
        if (progress < 0.0) {
            return 0.0;
        }
        if (progress > distanceMeters) {
            return distanceMeters;
        }

        return progress;
    }

    function drawDistanceMarker(dc, segment, distanceMeters, minLon, maxLat, scale, offsetX, offsetY, label) {
        var point = segment.pointAtDistanceMeters(distanceMeters);
        if (point == null) {
            return;
        }

        drawPreviewMarker(dc, point, minLon, maxLat, scale, offsetX, offsetY, label);
    }

    function drawPreviewMarker(dc, point, minLon, maxLat, scale, offsetX, offsetY, label) {
        var x = projectPreviewX(point, minLon, scale, offsetX).toNumber();
        var y = projectPreviewY(point, maxLat, scale, offsetY).toNumber();

        dc.drawLine(x, y - 5, x + 4, y + 4);
        dc.drawLine(x + 4, y + 4, x - 4, y + 4);
        dc.drawLine(x - 4, y + 4, x, y - 5);
        dc.drawText(x + 5, y, Graphics.FONT_TINY, label, Graphics.TEXT_JUSTIFY_LEFT);
    }

    function projectPreviewX(point, minLon, scale, offsetX) {
        return offsetX + ((pointFloat(point, :lon, "lon") - minLon) * scale);
    }

    function projectPreviewY(point, maxLat, scale, offsetY) {
        return offsetY + ((maxLat - pointFloat(point, :lat, "lat")) * scale);
    }

    function drawBox(dc, left, top, width, height) {
        var right = left + width;
        var bottom = top + height;

        dc.drawLine(left, top, right, top);
        dc.drawLine(right, top, right, bottom);
        dc.drawLine(right, bottom, left, bottom);
        dc.drawLine(left, bottom, left, top);
    }

    function runPendingMenuAction() {
        if (_pendingMenuAction == null) {
            return;
        }

        var action = _pendingMenuAction;
        _pendingMenuAction = null;

        if (action == :begin_pairing) {
            beginPairing();
        } else if (action == :reset_pairing) {
            resetPairing();
        } else if (action == :ride_summary) {
            showRideSummary();
        } else if (action == :browse_segments) {
            browseSegments();
        }
    }

    function drawInfoLine(dc, left, y, label, value, maxChars) {
        dc.drawText(
            left,
            y,
            Graphics.FONT_TINY,
            label + ": " + trimText(value, maxChars),
            Graphics.TEXT_JUSTIFY_LEFT
        );
    }

    function trimText(value, maxChars) {
        if (value == null) {
            return "-";
        }

        if (value.length() <= maxChars) {
            return value;
        }

        return value.substring(0, maxChars - 3) + "...";
    }

    function engineStatusEquals(expected) {
        var status = _engine.status();
        return status != null && status.equals(expected);
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

    function formatSeconds(value) {
        if (value == null) {
            return "-";
        }

        var whole = value.toNumber();
        var minutes = whole / 60;
        var seconds = whole % 60;
        return minutes.format("%d") + ":" + seconds.format("%02d");
    }

    function formatSignedSeconds(value) {
        if (value == null) {
            return "-";
        }

        var whole = value.toNumber();
        if (whole < 0) {
            return "-" + formatSeconds(0 - whole);
        }

        return "+" + formatSeconds(whole);
    }

    function rankLabel(result) {
        if (result == null || result.overallRank == null) {
            return "-";
        }

        if (result.overallRank > 10 && !result.overallRankExact) {
            return ">10";
        }

        if (!result.overallRankExact && result.overallRank == 10) {
            return "Top 10";
        }

        return "#" + result.overallRank.format("%d");
    }

    function pointValue(point, symbolKey, stringKey) {
        if (point == null) {
            return null;
        }

        var value = point[symbolKey];
        if (value != null) {
            return value;
        }

        return point[stringKey];
    }

    function pointFloat(point, symbolKey, stringKey) {
        var value = pointValue(point, symbolKey, stringKey);
        if (value == null) {
            return 0.0;
        }

        return value.toFloat();
    }
}

class SegmentDetailDelegate extends WatchUi.BehaviorDelegate {
    private var _view as SegmentDetailView;

    public function initialize(view as SegmentDetailView) {
        WatchUi.BehaviorDelegate.initialize();
        _view = view;
    }

    public function onSelect() {
        _view.toggleMap();
        return true;
    }

    public function onMenu() {
        return false;
    }
}

class SegmentDetailView extends WatchUi.View {
    const METERS_PER_MILE = 1609.344;
    const FEET_PER_METER = 3.28084;

    var _segment;
    var _showMap;

    function initialize(segment) {
        View.initialize();
        _segment = segment;
        _showMap = false;
    }

    function toggleMap() {
        _showMap = !_showMap;
        WatchUi.requestUpdate();
    }

    function onUpdate(dc) {
        var left = 12;
        var y = 12;
        var width = dc.getWidth();
        var height = dc.getHeight();
        var mapTop = 60;
        var mapHeight = height - mapTop - 16;

        dc.clear();
        dc.setColor(Graphics.COLOR_WHITE, Graphics.COLOR_BLACK);

        dc.drawText(left, y, Graphics.FONT_SMALL, trimText(_segment.title, 28), Graphics.TEXT_JUSTIFY_LEFT);
        y += 18;

        if (!_showMap) {
            drawInfoLine(dc, left, y, "Distance", formatDistance(_segment.distanceMeters), 18);
            y += 14;
            drawInfoLine(dc, left, y, "PR", formatSeconds(_segment.prSeconds), 12);
            y += 14;
            drawInfoLine(dc, left, y, "KOM", formatSeconds(_segment.komSeconds), 12);
            y += 14;
            drawInfoLine(dc, left, y, "Last", formatSeconds(_segment.lastAttemptSeconds), 12);
            y += 18;
            dc.drawText(left, y, Graphics.FONT_TINY, "Press Select to view map", Graphics.TEXT_JUSTIFY_LEFT);
        } else {
            drawRoutePreview(dc, _segment, left, mapTop, width - (left * 2), mapHeight);
        }
    }

    function drawRoutePreview(dc, segment, left, top, width, height) {
        drawBox(dc, left, top, width, height);

        var routePoints = segment.routePoints;
        if (routePoints == null || routePoints.size() < 2) {
            dc.drawText(left + 10, top + 18, Graphics.FONT_TINY, "No route preview", Graphics.TEXT_JUSTIFY_LEFT);
            return;
        }

        var minLat = pointFloat(routePoints[0], :lat, "lat");
        var maxLat = minLat;
        var minLon = pointFloat(routePoints[0], :lon, "lon");
        var maxLon = minLon;

        for (var i = 1; i < routePoints.size(); i += 1) {
            var lat = pointFloat(routePoints[i], :lat, "lat");
            var lon = pointFloat(routePoints[i], :lon, "lon");

            if (lat < minLat) {
                minLat = lat;
            }
            if (lat > maxLat) {
                maxLat = lat;
            }
            if (lon < minLon) {
                minLon = lon;
            }
            if (lon > maxLon) {
                maxLon = lon;
            }
        }

        var latSpan = maxLat - minLat;
        var lonSpan = maxLon - minLon;
        if (latSpan < 0.000001) {
            latSpan = 0.000001;
        }
        if (lonSpan < 0.000001) {
            lonSpan = 0.000001;
        }

        var padding = 10;
        var plotWidth = width - (padding * 2);
        var plotHeight = height - (padding * 2);
        var scaleX = plotWidth / lonSpan;
        var scaleY = plotHeight / latSpan;
        var scale = scaleX;
        if (scaleY < scale) {
            scale = scaleY;
        }

        var offsetX = left + padding + ((plotWidth - (lonSpan * scale)) / 2);
        var offsetY = top + padding + ((plotHeight - (latSpan * scale)) / 2);

        for (var pointIndex = 1; pointIndex < routePoints.size(); pointIndex += 1) {
            var previous = routePoints[pointIndex - 1];
            var current = routePoints[pointIndex];
            dc.drawLine(
                projectX(previous, minLon, scale, offsetX).toNumber(),
                projectY(previous, maxLat, scale, offsetY).toNumber(),
                projectX(current, minLon, scale, offsetX).toNumber(),
                projectY(current, maxLat, scale, offsetY).toNumber()
            );
        }

        drawMarker(dc, routePoints[0], minLon, maxLat, scale, offsetX, offsetY, "S");
        drawMarker(dc, routePoints[routePoints.size() - 1], minLon, maxLat, scale, offsetX, offsetY, "F");
    }

    function drawMarker(dc, point, minLon, maxLat, scale, offsetX, offsetY, label) {
        var x = projectX(point, minLon, scale, offsetX).toNumber();
        var y = projectY(point, maxLat, scale, offsetY).toNumber();

        dc.drawLine(x - 2, y, x + 2, y);
        dc.drawLine(x, y - 2, x, y + 2);
        dc.drawText(x + 6, y, Graphics.FONT_TINY, label, Graphics.TEXT_JUSTIFY_LEFT);
    }

    function projectX(point, minLon, scale, offsetX) {
        return offsetX + ((pointFloat(point, :lon, "lon") - minLon) * scale);
    }

    function projectY(point, maxLat, scale, offsetY) {
        return offsetY + ((maxLat - pointFloat(point, :lat, "lat")) * scale);
    }

    function drawBox(dc, left, top, width, height) {
        var right = left + width;
        var bottom = top + height;

        dc.drawLine(left, top, right, top);
        dc.drawLine(right, top, right, bottom);
        dc.drawLine(right, bottom, left, bottom);
        dc.drawLine(left, bottom, left, top);
    }

    function drawInfoLine(dc, left, y, label, value, maxChars) {
        dc.drawText(
            left,
            y,
            Graphics.FONT_TINY,
            label + ": " + trimText(value, maxChars),
            Graphics.TEXT_JUSTIFY_LEFT
        );
    }

    function trimText(value, maxChars) {
        if (value == null) {
            return "-";
        }

        if (value.length() <= maxChars) {
            return value;
        }

        return value.substring(0, maxChars - 3) + "...";
    }

    function pointValue(point, symbolKey, stringKey) {
        if (point == null) {
            return null;
        }

        var value = point[symbolKey];
        if (value != null) {
            return value;
        }

        return point[stringKey];
    }

    function pointFloat(point, symbolKey, stringKey) {
        var value = pointValue(point, symbolKey, stringKey);
        if (value == null) {
            return 0.0;
        }

        return value.toFloat();
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

    function formatSeconds(value) {
        if (value == null) {
            return "-";
        }

        var minutes = value / 60;
        var seconds = value % 60;
        return minutes.format("%d") + ":" + seconds.format("%02d");
    }
}

class EffortResultDetailDelegate extends WatchUi.BehaviorDelegate {
    private var _view as EffortResultDetailView;

    public function initialize(view as EffortResultDetailView) {
        WatchUi.BehaviorDelegate.initialize();
        _view = view;
    }

    public function onSelect() {
        _view.showSegmentMap();
        return true;
    }

    public function onMenu() {
        return false;
    }
}

class EffortResultDetailView extends WatchUi.View {
    const METERS_PER_MILE = 1609.344;
    const FEET_PER_METER = 3.28084;

    var _result;

    function initialize(result) {
        View.initialize();
        _result = result;
    }

    function showSegmentMap() {
        var detail = new $.SegmentDetailView(_result.segment);
        WatchUi.pushView(detail, new $.SegmentDetailDelegate(detail), WatchUi.SLIDE_LEFT);
    }

    function onUpdate(dc) {
        dc.clear();
        dc.setColor(Graphics.COLOR_WHITE, Graphics.COLOR_BLACK);

        var left = 12;
        var y = 12;
        var title = "Effort Result";
        if (_result.isKom) {
            title = "KOM Result";
        } else if (_result.isPr) {
            title = "PR Result";
        } else if (_result.isTop10) {
            title = "Top 10 Result";
        }

        dc.drawText(left, y, Graphics.FONT_SMALL, title, Graphics.TEXT_JUSTIFY_LEFT);
        y += 18;
        dc.drawText(left, y, Graphics.FONT_TINY, trimText(_result.segment.title, 34), Graphics.TEXT_JUSTIFY_LEFT);
        y += 20;

        drawInfoLine(dc, left, y, "Rank", rankLabel(_result), 16);
        y += 15;
        drawInfoLine(dc, left, y, "Time", formatSeconds(_result.elapsedSeconds), 16);
        y += 15;
        drawInfoLine(dc, left, y, "vs PR", formatSignedSeconds(_result.deltaPrSeconds), 16);
        y += 15;
        drawInfoLine(dc, left, y, "vs KOM", formatSignedSeconds(_result.deltaKomSeconds), 16);
        y += 15;
        drawInfoLine(dc, left, y, "vs T10", formatSignedSeconds(_result.deltaTop10Seconds), 16);
        y += 15;
        drawInfoLine(dc, left, y, "Distance", formatDistance(_result.segment.distanceMeters), 18);

        dc.drawText(left, dc.getHeight() - 18, Graphics.FONT_TINY, "Select for segment map", Graphics.TEXT_JUSTIFY_LEFT);
    }

    function drawInfoLine(dc, left, y, label, value, maxChars) {
        dc.drawText(
            left,
            y,
            Graphics.FONT_TINY,
            label + ": " + trimText(value, maxChars),
            Graphics.TEXT_JUSTIFY_LEFT
        );
    }

    function rankLabel(result) {
        if (result == null || result.overallRank == null) {
            return "-";
        }

        if (result.overallRank > 10 && !result.overallRankExact) {
            return ">10";
        }

        if (!result.overallRankExact && result.overallRank == 10) {
            return "Top 10";
        }

        return "#" + result.overallRank.format("%d");
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

    function formatSeconds(value) {
        if (value == null) {
            return "-";
        }

        var whole = value.toNumber();
        var minutes = whole / 60;
        var seconds = whole % 60;
        return minutes.format("%d") + ":" + seconds.format("%02d");
    }

    function formatSignedSeconds(value) {
        if (value == null) {
            return "-";
        }

        var whole = value.toNumber();
        if (whole < 0) {
            return "-" + formatSeconds(0 - whole);
        }

        return "+" + formatSeconds(whole);
    }

    function trimText(value, maxChars) {
        if (value == null) {
            return "-";
        }

        if (value.length() <= maxChars) {
            return value;
        }

        return value.substring(0, maxChars - 3) + "...";
    }
}

class RideSummaryDelegate extends WatchUi.BehaviorDelegate {
    private var _view as RideSummaryView;

    public function initialize(view as RideSummaryView) {
        WatchUi.BehaviorDelegate.initialize();
        _view = view;
    }

    public function onNextPage() {
        _view.showNext();
        return true;
    }

    public function onPreviousPage() {
        _view.showPrevious();
        return true;
    }

    public function onSelect() {
        _view.selectCurrent();
        return true;
    }

    public function onUp() {
        _view.showPrevious();
        return true;
    }

    public function onDown() {
        _view.showNext();
        return true;
    }
}

class RideSummaryView extends WatchUi.View {
    const ROW_HEIGHT = 23;

    var _engine;
    var _selectionIndex;
    var _lastTapX;
    var _lastTapY;
    var _lastTapCount;
    var _lastTapComputedSel;

    function initialize(engine) {
        View.initialize();
        _engine = engine;
        _selectionIndex = 0;
        _lastTapX = null;
        _lastTapY = null;
        _lastTapCount = 0;
        _lastTapComputedSel = -1;
    }

    function onUpdate(dc) {
        dc.clear();
        dc.setColor(Graphics.COLOR_WHITE, Graphics.COLOR_BLACK);

        var results = _engine.completedResults();
        var left = 12;
        var y = 16;

        if (results == null || results.size() == 0) {
            dc.drawText(left, y, Graphics.FONT_SMALL, "No completed segments", Graphics.TEXT_JUSTIFY_LEFT);
            return;
        }

        var visible = 5;
        var start = _selectionIndex - (visible / 2);
        if (start < 0) {
            start = 0;
        }
        if (start + visible > results.size()) {
            start = results.size() - visible;
            if (start < 0) {
                start = 0;
            }
        }

        for (var i = start; i < start + visible && i < results.size(); i += 1) {
            var res = results[i];
            var isSel = (i == _selectionIndex);
            var title = res.segment.title;
            var time = formatSeconds(res.elapsedSeconds);
            var delta = formatSignedSeconds(res.deltaPrSeconds);
            var flags = "";
            if (res.isKom) {
                flags = "KOM";
            } else if (res.isPr) {
                flags = "PR";
            } else if (res.isTop10) {
                flags = "T10";
            }
            var prefix = isSel ? "> " : "  ";
            dc.drawText(left, y, Graphics.FONT_SMALL, prefix + trimText(title, 22), Graphics.TEXT_JUSTIFY_LEFT);
            var rightText = rankLabel(res) + " " + time + " " + delta;
            if (flags.length() > 0) {
                rightText = rightText + " " + flags;
            }
            dc.drawText(dc.getWidth() - 4, y, Graphics.FONT_SMALL, trimText(rightText, 20), Graphics.TEXT_JUSTIFY_RIGHT);
            y += ROW_HEIGHT;
        }

        if (_lastTapCount > 0) {
            _lastTapCount -= 1;
        }
    }

    function showNext() {
        var results = _engine.completedResults();
        if (results == null || results.size() == 0) {
            return;
        }

        _selectionIndex = (_selectionIndex + 1) % results.size();
        WatchUi.requestUpdate();
    }

    function showPrevious() {
        var results = _engine.completedResults();
        if (results == null || results.size() == 0) {
            return;
        }

        _selectionIndex -= 1;
        if (_selectionIndex < 0) {
            _selectionIndex = results.size() - 1;
        }
        WatchUi.requestUpdate();
    }

    function currentResult() {
        var results = _engine.completedResults();
        if (results == null || results.size() == 0) {
            _selectionIndex = 0;
            return null;
        }

        if (_selectionIndex >= results.size()) {
            _selectionIndex = results.size() - 1;
        }

        return results[_selectionIndex];
    }

    function selectCurrent() {
        if (_lastTapComputedSel != null && _lastTapComputedSel >= 0) {
            _selectionIndex = _lastTapComputedSel;
            _lastTapComputedSel = -1;
        }

        var res = currentResult();
        if (res == null) {
            return;
        }

        var detail = new $.EffortResultDetailView(res);
        WatchUi.pushView(detail, new $.EffortResultDetailDelegate(detail), WatchUi.SLIDE_LEFT);
    }

    function onTap(x, y) {
        var results = _engine.completedResults();
        _lastTapX = x;
        _lastTapY = y;
        _lastTapCount = 6;

        if (results == null || results.size() == 0) {
            return true;
        }

        var visible = 5;
        var start = _selectionIndex - (visible / 2);
        if (start < 0) {
            start = 0;
        }
        if (start + visible > results.size()) {
            start = results.size() - visible;
            if (start < 0) {
                start = 0;
            }
        }

        var displayed = results.size() - start;
        if (displayed > visible) {
            displayed = visible;
        }

        var firstY = 16;

        if (y != null && y < firstY) {
            _selectionIndex -= visible;
            if (_selectionIndex < 0) {
                _selectionIndex = 0;
            }
            _lastTapComputedSel = _selectionIndex;
            WatchUi.requestUpdate();
            return true;
        }

        var lastRowBottom = firstY + (displayed * ROW_HEIGHT);
        if (y != null && y >= lastRowBottom) {
            _selectionIndex += visible;
            var maxIndex = results.size() - 1;
            if (_selectionIndex > maxIndex) {
                _selectionIndex = maxIndex;
            }
            _lastTapComputedSel = _selectionIndex;
            WatchUi.requestUpdate();
            return true;
        }

        if (y == null) {
            return true;
        }

        for (var j = 0; j < displayed && start + j < results.size(); j += 1) {
            var itemTop = firstY + (j * ROW_HEIGHT);
            var itemBottom = itemTop + ROW_HEIGHT;
            if (y >= itemTop && y < itemBottom) {
                _selectionIndex = start + j;
                _lastTapComputedSel = _selectionIndex;
                WatchUi.requestUpdate();
                return true;
            }
        }

        _lastTapComputedSel = -1;
        return true;
    }

    function formatSeconds(value) {
        if (value == null) {
            return "-";
        }

        var whole = value.toNumber();
        var minutes = whole / 60;
        var seconds = whole % 60;
        return minutes.format("%d") + ":" + seconds.format("%02d");
    }

    function formatSignedSeconds(value) {
        if (value == null) {
            return "-";
        }

        var whole = value.toNumber();
        if (whole < 0) {
            return "-" + formatSeconds(0 - whole);
        }

        return "+" + formatSeconds(whole);
    }

    function rankLabel(result) {
        if (result == null || result.overallRank == null) {
            return "-";
        }

        if (result.overallRank > 10 && !result.overallRankExact) {
            return ">10";
        }

        if (!result.overallRankExact && result.overallRank == 10) {
            return "T10";
        }

        return "#" + result.overallRank.format("%d");
    }

    function trimText(value, maxChars) {
        if (value == null) {
            return "-";
        }

        if (value.length() <= maxChars) {
            return value;
        }

        return value.substring(0, maxChars - 3) + "...";
    }
}

class BikeSegmentsDelegate extends WatchUi.BehaviorDelegate {
    private var _view as BikeSegmentsView;

    public function initialize(view as BikeSegmentsView) {
        WatchUi.BehaviorDelegate.initialize();
        _view = view;
    }

    public function onMenu() {
        var menuView = new $.BikeSegmentsMenuView(_view);
        var delegate = new $.BikeSegmentsMenuDelegate(menuView);
        WatchUi.pushView(menuView, delegate, WatchUi.SLIDE_IMMEDIATE);
        return true;
    }

    public function onSelect() {
        return _view.handlePrimarySelect();
    }

    public function onUp() {
        return _view.showPreviousActive();
    }

    public function onDown() {
        return _view.showNextActive();
    }

    public function onPreviousPage() {
        return _view.showPreviousActive();
    }

    public function onNextPage() {
        return _view.showNextActive();
    }
}

class BikeSegmentsMenuView extends WatchUi.View {
    const FIRST_ROW_Y = 46;
    const ROW_HEIGHT = 28;

    var _homeView;
    var _selectionIndex;

    public function initialize(homeView as BikeSegmentsView) {
        View.initialize();
        _homeView = homeView;
        _selectionIndex = 0;
    }

    function onUpdate(dc) {
        dc.clear();
        dc.setColor(Graphics.COLOR_WHITE, Graphics.COLOR_BLACK);

        var left = 12;
        var y = 16;

        dc.drawText(left, y, Graphics.FONT_SMALL, "Bike Segments", Graphics.TEXT_JUSTIFY_LEFT);
        y = FIRST_ROW_Y;

        for (var i = 0; i < itemCount(); i += 1) {
            var prefix = i == _selectionIndex ? "> " : "  ";
            dc.drawText(left, y, Graphics.FONT_SMALL, prefix + trimText(itemLabel(i), 26), Graphics.TEXT_JUSTIFY_LEFT);
            y += ROW_HEIGHT;
        }

        dc.drawText(left, dc.getHeight() - 18, Graphics.FONT_TINY, "Tap or press Select", Graphics.TEXT_JUSTIFY_LEFT);
    }

    function showNext() {
        _selectionIndex = (_selectionIndex + 1) % itemCount();
        WatchUi.requestUpdate();
    }

    function showPrevious() {
        _selectionIndex -= 1;
        if (_selectionIndex < 0) {
            _selectionIndex = itemCount() - 1;
        }

        WatchUi.requestUpdate();
    }

    function selectCurrent() {
        _homeView.queueMenuAction(itemAction(_selectionIndex));
        WatchUi.popView(WatchUi.SLIDE_IMMEDIATE);
    }

    function onTap(x, y) {
        if (y == null || y < FIRST_ROW_Y) {
            return false;
        }

        var itemIndex = (y - FIRST_ROW_Y) / ROW_HEIGHT;
        if (itemIndex < 0 || itemIndex >= itemCount()) {
            return false;
        }

        _selectionIndex = itemIndex;
        selectCurrent();
        return true;
    }

    function itemCount() {
        return 4;
    }

    function itemLabel(index) {
        if (index == 0) {
            return _homeView.browserMenuLabel();
        }

        if (index == 1) {
            return "Segment Summary";
        }

        if (index == 2) {
            return "Pair";
        }

        return "Reset Pairing";
    }

    function itemAction(index) {
        if (index == 0) {
            return :browse_segments;
        }

        if (index == 1) {
            return :ride_summary;
        }

        if (index == 2) {
            return :begin_pairing;
        }

        return :reset_pairing;
    }

    function trimText(value, maxChars) {
        if (value == null) {
            return "-";
        }

        if (value.length() <= maxChars) {
            return value;
        }

        return value.substring(0, maxChars - 3) + "...";
    }
}

class BikeSegmentsMenuDelegate extends WatchUi.BehaviorDelegate {
    private var _view as BikeSegmentsMenuView;

    public function initialize(view as BikeSegmentsMenuView) {
        WatchUi.BehaviorDelegate.initialize();
        _view = view;
    }

    public function onSelect() {
        _view.selectCurrent();
        return true;
    }

    public function onUp() {
        _view.showPrevious();
        return true;
    }

    public function onDown() {
        _view.showNext();
        return true;
    }

    public function onPreviousPage() {
        _view.showPrevious();
        return true;
    }

    public function onNextPage() {
        _view.showNext();
        return true;
    }

    public function onTap(event) {
        if (event == null) {
            return false;
        }

        var coords = event.getCoordinates();
        if (coords == null || coords.size() < 2) {
            return false;
        }

        return _view.onTap(coords[0], coords[1]);
    }
}

class SegmentBrowserDelegate extends WatchUi.BehaviorDelegate {
    private var _view as SegmentBrowserView;

    public function initialize(view as SegmentBrowserView) {
        WatchUi.BehaviorDelegate.initialize();
        _view = view;
    }

    public function onNextPage() {
        _view.showNext();
        return true;
    }

    public function onPreviousPage() {
        _view.showPrevious();
        return true;
    }

    public function onSelect() {
        _view.selectCurrent();
        return true;
    }

    public function onUp() {
        _view.showPrevious();
        return true;
    }

    public function onDown() {
        _view.showNext();
        return true;
    }

    public function onTap(event) {
        if (event != null) {
            // event may expose getX()/getY() methods or be array-like
            var coords = event.getCoordinates();
            if (coords != null && coords.size() >= 2) {
                return _view.onTap(coords[0], coords[1]);
            }

            if (event[0] != null) {
                return _view.onTap(event[0], event[1]);
            }

            return false;
        }

        return false;
    }
}

class SegmentBrowserView extends WatchUi.View {

    const METERS_PER_MILE = 1609.344;
    const FEET_PER_METER = 3.28084;
    const ROW_HEIGHT = 23;

    var _sync;
    var _selectionIndex;
    var _lastTapX;
    var _lastTapY;
    var _lastTapCount;
    var _lastTapComputedSel;

    function initialize(sync) {
        View.initialize();
        _sync = sync;
        _selectionIndex = 0;
        _lastTapX = null;
        _lastTapY = null;
        _lastTapCount = 0;
        _lastTapComputedSel = -1;
    }

    function onUpdate(dc) {
        var segments = _sync.segments();
        var width = dc.getWidth();
        var height = dc.getHeight();
        var left = 12;
        var y = 16;
        var listHeight = height - 24;

        dc.clear();
        dc.setColor(Graphics.COLOR_WHITE, Graphics.COLOR_BLACK);

        if (segments.size() == 0) {
            dc.drawText(left, y, Graphics.FONT_SMALL, "No synced segments", Graphics.TEXT_JUSTIFY_LEFT);
            dc.drawText(left, y + 20, Graphics.FONT_TINY, "Pair and sync to browse routes.", Graphics.TEXT_JUSTIFY_LEFT);
            return;
        }

        var visible = 5;
        var start = _selectionIndex - (visible / 2);
        if (start < 0) {
            start = 0;
        }
        if (start + visible > segments.size()) {
            start = segments.size() - visible;
            if (start < 0) {
                start = 0;
            }
        }

        for (var i = start; i < start + visible && i < segments.size(); i += 1) {
            var seg = segments[i];
            var isSel = (i == _selectionIndex);
            var title = trimText(seg.title, 28);
            var prefix = isSel ? "> " : "  ";
            dc.drawText(left, y, Graphics.FONT_SMALL, prefix + title, Graphics.TEXT_JUSTIFY_LEFT);
            y += ROW_HEIGHT;
        }

        dc.drawText(left, height - 18, Graphics.FONT_TINY, "Press Select to view on map", Graphics.TEXT_JUSTIFY_LEFT);

        if (_lastTapCount > 0) {
            var tx = _lastTapX == null ? "-" : _lastTapX.format("%d");
            var ty = _lastTapY == null ? "-" : _lastTapY.format("%d");
            var calc = _lastTapComputedSel < 0 ? "-" : _lastTapComputedSel.format("%d");
            dc.drawText(left, height - 34, Graphics.FONT_TINY, "tap: " + tx + "," + ty + " sel: " + _selectionIndex.format("%d") + " calc: " + calc, Graphics.TEXT_JUSTIFY_LEFT);
            _lastTapCount -= 1;
        }
    }

    function showNext() {
        var segments = _sync.segments();
        if (segments.size() == 0) {
            return;
        }

        _selectionIndex = (_selectionIndex + 1) % segments.size();
        WatchUi.requestUpdate();
    }

    function showPrevious() {
        var segments = _sync.segments();
        if (segments.size() == 0) {
            return;
        }

        _selectionIndex -= 1;
        if (_selectionIndex < 0) {
            _selectionIndex = segments.size() - 1;
        }
        WatchUi.requestUpdate();
    }

    function currentSegment() {
        var segments = _sync.segments();
        if (segments.size() == 0) {
            _selectionIndex = 0;
            return null;
        }

        if (_selectionIndex >= segments.size()) {
            _selectionIndex = segments.size() - 1;
        }

        return segments[_selectionIndex];
    }

    function selectCurrent() {
        // If a recent tap computed a selection, prefer it
        if (_lastTapComputedSel != null && _lastTapComputedSel >= 0) {
            _selectionIndex = _lastTapComputedSel;
            _lastTapComputedSel = -1;
        }

        var seg = currentSegment();
        if (seg == null) {
            return;
        }

        var detail = new $.SegmentDetailView(seg);
        WatchUi.pushView(detail, new $.SegmentDetailDelegate(detail), WatchUi.SLIDE_LEFT);
    }

    function onTap(x, y) {
        var segments = _sync.segments();
        _lastTapX = x;
        _lastTapY = y;
        _lastTapCount = 6;

        if (segments.size() == 0) {
            return true;
        }

        var visible = 5;
        var start = _selectionIndex - (visible / 2);
        if (start < 0) {
            start = 0;
        }
        if (start + visible > segments.size()) {
            start = segments.size() - visible;
            if (start < 0) {
                start = 0;
            }
        }

        var displayed = segments.size() - start;
        if (displayed > visible) {
            displayed = visible;
        }

        var firstY = 16;

        // Tap above the list pages up
        if (y != null && y < firstY) {
            _selectionIndex -= visible;
            if (_selectionIndex < 0) {
                _selectionIndex = 0;
            }
            _lastTapComputedSel = _selectionIndex;
            WatchUi.requestUpdate();
            return true;
        }

        // Tap below the list pages down
        var lastRowBottom = firstY + (displayed * ROW_HEIGHT);
        if (y != null && y >= lastRowBottom) {
            _selectionIndex += visible;
            if (_selectionIndex >= segments.size()) {
                _selectionIndex = segments.size() - 1;
            }
            _lastTapComputedSel = _selectionIndex;
            WatchUi.requestUpdate();
            return true;
        }

        if (y == null) {
            // No coordinates available; consume event to avoid default selection
            return true;
        }

        for (var j = 0; j < displayed && start + j < segments.size(); j += 1) {
            var itemTop = firstY + (j * ROW_HEIGHT);
            var itemBottom = itemTop + ROW_HEIGHT;
            if (y >= itemTop && y < itemBottom) {
                _selectionIndex = start + j;
                _lastTapComputedSel = _selectionIndex;
                WatchUi.requestUpdate();
                return true;
            }
        }

        // If no row matched, still consume the event and clear computed
        _lastTapComputedSel = -1;
        return true;
    }

    function drawRoutePreview(dc, segment, left, top, width, height) {
        drawBox(dc, left, top, width, height);

        var routePoints = segment.routePoints;
        if (routePoints == null || routePoints.size() < 2) {
            dc.drawText(left + 10, top + 18, Graphics.FONT_TINY, "No route preview", Graphics.TEXT_JUSTIFY_LEFT);
            return;
        }

        var minLat = pointFloat(routePoints[0], :lat, "lat");
        var maxLat = minLat;
        var minLon = pointFloat(routePoints[0], :lon, "lon");
        var maxLon = minLon;

        for (var i = 1; i < routePoints.size(); i += 1) {
            var lat = pointFloat(routePoints[i], :lat, "lat");
            var lon = pointFloat(routePoints[i], :lon, "lon");

            if (lat < minLat) {
                minLat = lat;
            }
            if (lat > maxLat) {
                maxLat = lat;
            }
            if (lon < minLon) {
                minLon = lon;
            }
            if (lon > maxLon) {
                maxLon = lon;
            }
        }

        var latSpan = maxLat - minLat;
        var lonSpan = maxLon - minLon;
        if (latSpan < 0.000001) {
            latSpan = 0.000001;
        }
        if (lonSpan < 0.000001) {
            lonSpan = 0.000001;
        }

        var padding = 10;
        var plotWidth = width - (padding * 2);
        var plotHeight = height - (padding * 2);
        var scaleX = plotWidth / lonSpan;
        var scaleY = plotHeight / latSpan;
        var scale = scaleX;
        if (scaleY < scale) {
            scale = scaleY;
        }

        var offsetX = left + padding + ((plotWidth - (lonSpan * scale)) / 2);
        var offsetY = top + padding + ((plotHeight - (latSpan * scale)) / 2);

        for (var pointIndex = 1; pointIndex < routePoints.size(); pointIndex += 1) {
            var previous = routePoints[pointIndex - 1];
            var current = routePoints[pointIndex];
            dc.drawLine(
                projectX(previous, minLon, scale, offsetX).toNumber(),
                projectY(previous, maxLat, scale, offsetY).toNumber(),
                projectX(current, minLon, scale, offsetX).toNumber(),
                projectY(current, maxLat, scale, offsetY).toNumber()
            );
        }

        drawMarker(dc, routePoints[0], minLon, maxLat, scale, offsetX, offsetY, "S");
        drawMarker(dc, routePoints[routePoints.size() - 1], minLon, maxLat, scale, offsetX, offsetY, "F");
    }

    function drawMarker(dc, point, minLon, maxLat, scale, offsetX, offsetY, label) {
        var x = projectX(point, minLon, scale, offsetX).toNumber();
        var y = projectY(point, maxLat, scale, offsetY).toNumber();

        dc.drawLine(x - 2, y, x + 2, y);
        dc.drawLine(x, y - 2, x, y + 2);
        dc.drawText(x + 6, y, Graphics.FONT_TINY, label, Graphics.TEXT_JUSTIFY_LEFT);
    }

    function projectX(point, minLon, scale, offsetX) {
        return offsetX + ((pointFloat(point, :lon, "lon") - minLon) * scale);
    }

    function projectY(point, maxLat, scale, offsetY) {
        return offsetY + ((maxLat - pointFloat(point, :lat, "lat")) * scale);
    }

    function drawBox(dc, left, top, width, height) {
        var right = left + width;
        var bottom = top + height;

        dc.drawLine(left, top, right, top);
        dc.drawLine(right, top, right, bottom);
        dc.drawLine(right, bottom, left, bottom);
        dc.drawLine(left, bottom, left, top);
    }

    function drawInfoLine(dc, left, y, label, value, maxChars) {
        dc.drawText(
            left,
            y,
            Graphics.FONT_TINY,
            label + ": " + trimText(value, maxChars),
            Graphics.TEXT_JUSTIFY_LEFT
        );
    }

    function trimText(value, maxChars) {
        if (value == null) {
            return "-";
        }

        if (value.length() <= maxChars) {
            return value;
        }

        return value.substring(0, maxChars - 3) + "...";
    }

    function pointValue(point, symbolKey, stringKey) {
        if (point == null) {
            return null;
        }

        var value = point[symbolKey];
        if (value != null) {
            return value;
        }

        return point[stringKey];
    }

    function pointFloat(point, symbolKey, stringKey) {
        var value = pointValue(point, symbolKey, stringKey);
        if (value == null) {
            return 0.0;
        }

        return value.toFloat();
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

    function formatSeconds(value) {
        if (value == null) {
            return "-";
        }

        var minutes = value / 60;
        var seconds = value % 60;
        return minutes.format("%d") + ":" + seconds.format("%02d");
    }
}
