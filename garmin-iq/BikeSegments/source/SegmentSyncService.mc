using Toybox.Communications;
using Toybox.Application;
using Toybox.Lang;
using Toybox.Math;
using Toybox.PersistedContent;
using Toybox.Time;

class SegmentSyncService {

    const SYNC_CONFIG = {
        :begin_link_path => "/api/garmin-iq/link/begin",
        :poll_link_path => "/api/garmin-iq/link/poll",
        :reset_path => "/api/garmin-iq/link/reset",
        :refresh_path => "/api/garmin-iq/auth/refresh",
        :sync_path => "/api/garmin-iq/segments/sync"
    };

    // These are fallback defaults only. In normal builds, Rez.Strings values
    // from resources/strings/*.xml are loaded first and take precedence.
    const API_BASE_URL_LOCAL = "http://localhost:3000";
    const API_BASE_URL_PROD = "https://bike.nibelheim.dev";
    const ACCOUNT_URL_LOCAL = "http://localhost:3001/account";
    const ACCOUNT_URL_PROD = "https://bike.nibelheim.dev/account";
    const ACCOUNT_LABEL_LOCAL = "localhost:3001/account";
    const ACCOUNT_LABEL_PROD = "bike.nibelheim.dev/account";

    const STORAGE_KEYS = {
        :api_base_url => "garmin_iq_api_base_url",
        :install_id => "garmin_iq_install_id",
        :pairing_code_prefix => "garmin_iq_pairing_code_",
        :refresh_token_prefix => "garmin_iq_refresh_token_",
        :access_token_prefix => "garmin_iq_access_token_"
    };

    var _segments;
    var _lastSyncStatus;
    var _apiBaseUrl;
    var _accountUrl;
    var _accountLabel;
    var _verificationUrl;
    var _installId;
    var _pairingCode;
    var _refreshToken;
    var _accessToken;
    var _isBusy;
    var _lastError;
    var _debugEvents;
    var _authFailureCount;
    var _allowProductionFallback;

    function initialize() {
        _segments = [];
        _lastSyncStatus = "never";
        var resApi = loadStringResource(Rez.Strings.ApiBaseUrl);
        var resAccountUrl = loadStringResource(Rez.Strings.AccountUrl);
        var resAccountLabel = loadStringResource(Rez.Strings.AccountLabel);
        var resAllowProductionFallback = loadStringResource(Rez.Strings.AllowProductionFallback);
        var storedApi = loadStoredValue(STORAGE_KEYS[:api_base_url]);

        if (resApi != null) {
            _apiBaseUrl = resApi;
            storeValue(STORAGE_KEYS[:api_base_url], _apiBaseUrl);
        } else if (storedApi != null) {
            _apiBaseUrl = storedApi;
        } else {
            _apiBaseUrl = API_BASE_URL_PROD;
        }

        if (resAccountUrl != null) {
            _accountUrl = resAccountUrl;
        } else {
            _accountUrl = defaultAccountUrlFor(_apiBaseUrl);
        }

        if (resAccountLabel != null) {
            _accountLabel = resAccountLabel;
        } else {
            _accountLabel = defaultAccountLabelFor(_apiBaseUrl);
        }

        _verificationUrl = _accountUrl;
        _installId = loadOrCreateInstallId();
        loadAuthState();
        _isBusy = false;
        _lastError = null;
        _debugEvents = [];
        _authFailureCount = 0;
        _allowProductionFallback = stringEquals(resAllowProductionFallback, "true");
        recordEvent("ready " + apiEnvironmentLabel());
    }

    function segments() {
        return _segments;
    }

    function lastSyncStatus() {
        return _lastSyncStatus;
    }

    function lastError() {
        return _lastError;
    }

    function debugLines() {
        return _debugEvents;
    }

    function linkCode() {
        return _pairingCode;
    }

    function apiEnvironmentLabel() {
        return apiLabelFor(_apiBaseUrl);
    }

    function verificationLabel() {
        if (stringEquals(_verificationUrl, ACCOUNT_URL_LOCAL)) {
            return ACCOUNT_LABEL_LOCAL;
        }

        if (stringEquals(_verificationUrl, ACCOUNT_URL_PROD)) {
            return ACCOUNT_LABEL_PROD;
        }

        return _accountLabel;
    }

    function syncNow() {
        if (_isBusy) {
            return;
        }

        // Only continue existing flows. Do not automatically begin a new pairing
        // when the app launches — pairing should be started explicitly from the menu.
        if (_refreshToken != null) {
            if (_accessToken == null) {
                refreshAccessToken();
                return;
            }

            fetchSegments();
            return;
        }

        if (_pairingCode != null) {
            pollLink();
            return;
        }

        _lastSyncStatus = "idle";
    }

    function beginLink() {
        if (_isBusy) {
            _lastSyncStatus = "busy";
            return;
        }

        _isBusy = true;
        _lastSyncStatus = "begin link";
        var url = _apiBaseUrl
            + SYNC_CONFIG[:begin_link_path]
            + "?install_id=" + _installId
            + "&device_name=BikeSegments";

        recordEvent("begin link");

        Communications.makeWebRequest(
            url,
            null,
            {
                :method => Communications.HTTP_REQUEST_METHOD_GET,
                :responseType => Communications.HTTP_RESPONSE_CONTENT_TYPE_JSON
            },
            method(:onBeginLinkResponse)
        );
    }

    function onBeginLinkResponse(code as Lang.Number, data as Lang.Dictionary or Lang.String or PersistedContent.Iterator or Null) as Void {
        _isBusy = false;
        if (shouldFallback(code)) {
            switchToProductionApi();
            beginLink();
            return;
        }

        if (code != 200 || data == null || !(data instanceof Lang.Dictionary)) {
            _lastSyncStatus = "link error " + code.format("%d");
            _lastError = "beginLink response invalid: code=" + code.format("%d");
            storeValue("garmin_iq_last_error", _lastError);
            recordEvent("begin failed " + code.format("%d"));
            return;
        }

        var payload = data;
        var verificationUrl = jsonStringValue(payload, :verification_url, "verification_url");
        if (verificationUrl != null) {
            _verificationUrl = verificationUrl;
        } else {
            _verificationUrl = _accountUrl;
        }

        _pairingCode = jsonStringValue(payload, :pairing_code, "pairing_code");
        if (_pairingCode != null) {
            storePairingCode(_pairingCode);
            _lastSyncStatus = "link code " + _pairingCode;
            _authFailureCount = 0;
            _lastError = null;
            storeValue("garmin_iq_last_error", null);
            recordEvent("code " + _pairingCode);
        } else {
            _lastSyncStatus = "link failed";
            _lastError = "beginLink: no pairing code in response";
            storeValue("garmin_iq_last_error", _lastError);
            recordEvent("begin no code");
        }
    }

    function pollLink() {
        if (_pairingCode == null) {
            // nothing to poll
            _lastSyncStatus = "idle";
            return;

        }

        if (_isBusy) {
            _lastSyncStatus = "busy";
            return;
        }

        _isBusy = true;
        _lastSyncStatus = "poll link";
        var url = _apiBaseUrl
            + SYNC_CONFIG[:poll_link_path]
            + "?install_id=" + _installId
            + "&pairing_code=" + _pairingCode;

        Communications.makeWebRequest(
            url,
            null,
            {
                :method => Communications.HTTP_REQUEST_METHOD_GET,
                :responseType => Communications.HTTP_RESPONSE_CONTENT_TYPE_JSON
            },
            method(:onPollLinkResponse)
        );
    }

    function onPollLinkResponse(code as Lang.Number, data as Lang.Dictionary or Lang.String or PersistedContent.Iterator or Null) as Void {
        _isBusy = false;
        if (shouldFallback(code)) {
            switchToProductionApi();
            pollLink();
            return;
        }
        // Handle specific non-200 responses that indicate the pairing state
        // on the server is invalid (e.g. pairing record deleted/cleared).
        if (code != 200 || data == null) {
            if (data != null && (data instanceof Lang.Dictionary)) {
                var errMsg = jsonStringValue(data, :message, "message");
                if (errMsg != null && (errMsg instanceof Lang.String)) {
                    if (stringEquals(errMsg, "Invalid pairing state") || stringEquals(errMsg, "invalid pairing state")) {
                        // Reset local pairing state and wait for user to begin pairing.
                        clearLinkState();
                        _lastSyncStatus = "link reset";
                        return;
                    }
                }
            }
            _lastSyncStatus = "poll error " + code.format("%d");
            _lastError = "pollLink error: code=" + code.format("%d");
            storeValue("garmin_iq_last_error", _lastError);
            recordEvent("poll invalid " + code.format("%d"));
            recordEvent("poll failed " + code.format("%d"));
            return;
        }

        if (!(data instanceof Lang.Dictionary)) {
            _lastSyncStatus = "poll error " + code.format("%d");
            recordEvent("poll non-dict");
            return;
        }

        var payload = data;
        var status = jsonStringValue(payload, :status, "status");

        if (status == null) {
            _lastSyncStatus = "poll status missing";
            recordEvent("poll no status");
            return;
        }

        if (stringEquals(status, "pending")) {
            _lastSyncStatus = "waiting approval " + _pairingCode;
            recordEvent("waiting approval");
            return;
        }

        if (stringEquals(status, "expired")) {
            clearLinkState();
            _lastSyncStatus = "code expired";
            _lastError = "pairing code expired";
            storeValue("garmin_iq_last_error", _lastError);
            recordEvent("code expired");
            return;
        }

        if (stringEquals(status, "linked")) {
            var linkedRefreshToken = jsonStringValue(payload, :refresh_token, "refresh_token");
            var linkedAccessToken = jsonStringValue(payload, :access_token, "access_token");

            if (linkedRefreshToken != null) {
                _refreshToken = linkedRefreshToken;
                storeRefreshToken(_refreshToken);
            }

            if (linkedAccessToken != null) {
                _accessToken = linkedAccessToken;
                storeAccessToken(_accessToken);
            }

            _authFailureCount = 0;
            _pairingCode = null;
            storePairingCode(null);

            if (_refreshToken == null && _accessToken == null) {
                _lastSyncStatus = "linked reset";
                _lastError = "linked without tokens; resetting";
                storeValue("garmin_iq_last_error", _lastError);
                recordEvent("linked no token");
                requestReset();
                return;
            }

            _lastSyncStatus = "linked";
            _lastError = null;
            storeValue("garmin_iq_last_error", null);
            recordEvent("linked");
            fetchSegments();
            return;
        }

        _lastSyncStatus = "poll " + status;
        recordEvent("poll " + status);
    }

    function refreshAccessToken() {
        if (_refreshToken == null) {
            clearLinkState();
            _lastSyncStatus = "refresh missing";
            _lastError = "refresh missing token";
            storeValue("garmin_iq_last_error", _lastError);
            recordEvent("refresh missing");
            return;
        }

        if (_isBusy) {
            _lastSyncStatus = "busy";
            return;
        }

        _isBusy = true;
        _lastSyncStatus = "refresh";
        var url = _apiBaseUrl
            + SYNC_CONFIG[:refresh_path]
            + "?install_id=" + _installId
            + "&refresh_token=" + _refreshToken;

        Communications.makeWebRequest(
            url,
            null,
            {
                :method => Communications.HTTP_REQUEST_METHOD_GET,
                :responseType => Communications.HTTP_RESPONSE_CONTENT_TYPE_JSON
            },
            method(:onRefreshResponse)
        );
    }

    function onRefreshResponse(code as Lang.Number, data as Lang.Dictionary or Lang.String or PersistedContent.Iterator or Null) as Void {
        _isBusy = false;
        if (shouldFallback(code)) {
            switchToProductionApi();
            refreshAccessToken();
            return;
        }
        if (code != 200 || data == null || !(data instanceof Lang.Dictionary)) {
            clearAuthTokens();
            _lastSyncStatus = "refresh error " + code.format("%d");
            _lastError = "refresh failed: code=" + code.format("%d");
            storeValue("garmin_iq_last_error", _lastError);
            recordEvent("refresh failed " + code.format("%d"));
            return;
        }

        var payload = data;
        _accessToken = jsonStringValue(payload, :access_token, "access_token");
        if (_accessToken != null) {
            storeAccessToken(_accessToken);
            _lastError = null;
            storeValue("garmin_iq_last_error", null);
            fetchSegments();
        } else {
            _lastSyncStatus = "refresh failed";
            _lastError = "refresh failed: no access token";
            storeValue("garmin_iq_last_error", _lastError);
            recordEvent("refresh no token");
        }
    }

    function fetchSegments() {
        if (_accessToken == null) {
            refreshAccessToken();
            return;
        }

        if (_isBusy) {
            _lastSyncStatus = "busy";
            return;
        }

        _isBusy = true;
        _lastSyncStatus = "sync";
        var url = _apiBaseUrl + SYNC_CONFIG[:sync_path];
        var headers = {
            "Authorization" => "Bearer " + _accessToken,
            "Content-Type" => "application/json"
        };

        Communications.makeWebRequest(
            url,
            null,
            {
                :method => Communications.HTTP_REQUEST_METHOD_GET,
                :headers => headers,
                :responseType => Communications.HTTP_RESPONSE_CONTENT_TYPE_JSON
            },
            method(:onSyncResponse)
        );
    }

    function onSyncResponse(code as Lang.Number, data as Lang.Dictionary or Lang.String or PersistedContent.Iterator or Null) as Void {
        _isBusy = false;

        if (code == 401) {
            _accessToken = null;
            storeAccessToken(null);
            _authFailureCount += 1;
            recordEvent("sync 401");
            if (_authFailureCount >= 2) {
                recordEvent("auth failures, requesting reset");
                requestReset();
                return;
            }
            refreshAccessToken();
            return;
        }

        if (shouldFallback(code)) {
            switchToProductionApi();
            fetchSegments();
            return;
        }

        if (code != 200 || data == null) {
            // Record API URL along with the error code to help diagnose
            // transient network/VNC/simulator issues (e.g. code=-200).
            var errMsg = "sync error: code=" + code.format("%d") + " api=" + _apiBaseUrl;
            _lastSyncStatus = "error " + code.format("%d");
            _lastError = errMsg;
            storeValue("garmin_iq_last_error", _lastError);
            recordEvent("sync failed " + code.format("%d") + " api=" + _apiBaseUrl);
            return;
        }

        if (!(data instanceof Lang.Dictionary)) {
            _lastSyncStatus = "error response";
            return;
        }

        var payload = data;
        var rawSegments = jsonValue(payload, :segments, "segments");
        if (rawSegments == null || !(rawSegments instanceof Lang.Array)) {
            _lastSyncStatus = "error response";
            _lastError = "sync response missing segments";
            storeValue("garmin_iq_last_error", _lastError);
            recordEvent("sync no segments");
            return;
        }

        var parsed = [];

        for (var i = 0; i < rawSegments.size(); i += 1) {
            parsed.add(new SegmentModel(rawSegments[i]));
        }

        _segments = parsed;
        // Debug: record how many segments we received and a sample of the first
        var syncedAt = jsonStringValue(payload, :synced_at, "synced_at");
        recordEvent("segments " + rawSegments.size().format("%d"));
        if (rawSegments.size() > 0) {
            var firstRaw = rawSegments[0];
            var firstId = jsonStringValue(firstRaw, :id, "id");
            var firstTitle = jsonStringValue(firstRaw, :title, "title");
            if (firstId != null) {
                recordEvent("first id " + firstId);
            } else if (firstTitle != null) {
                recordEvent("first title " + firstTitle);
            }
            var rp = jsonValue(firstRaw, :route_points, "route_points");
            if (rp == null || !(rp instanceof Lang.Array)) {
                recordEvent("first rpoints 0");
            } else {
                recordEvent("first rpoints " + rp.size().format("%d"));
            }
        }
        if (syncedAt == null) {
            syncedAt = "unknown";
        }

        _lastSyncStatus = "ok " + syncedAt;
        _lastError = null;
        storeValue("garmin_iq_last_error", null);
        recordEvent("sync ok");
    }

    function clearAuthTokens() {
        _accessToken = null;
        _refreshToken = null;
        storeAccessToken(null);
        storeRefreshToken(null);
    }

    function clearLinkState() {
        clearAuthTokens();
        _pairingCode = null;
        storePairingCode(null);
    }

    function requestReset() {
        if (_isBusy) {
            _lastSyncStatus = "busy";
            return;
        }

        _isBusy = true;
        _lastSyncStatus = "resetting";
        var url = _apiBaseUrl
            + SYNC_CONFIG[:reset_path]
            + "?install_id=" + _installId;

        recordEvent("reset request");

        Communications.makeWebRequest(
            url,
            null,
            {
                :method => Communications.HTTP_REQUEST_METHOD_POST,
                :responseType => Communications.HTTP_RESPONSE_CONTENT_TYPE_JSON
            },
            method(:onResetResponse)
        );
    }

    function onResetResponse(code as Lang.Number, data as Lang.Dictionary or Lang.String or PersistedContent.Iterator or Null) as Void {
        _isBusy = false;
        if (shouldFallback(code)) {
            switchToProductionApi();
            requestReset();
            return;
        }

        if (code == 200) {
            clearLinkState();
            _lastSyncStatus = "reset";
            _lastError = null;
            storeValue("garmin_iq_last_error", null);
            recordEvent("reset ok");
            return;
        }

        _lastSyncStatus = "reset error " + code.format("%d");
        _lastError = "reset failed: code=" + code.format("%d");
        storeValue("garmin_iq_last_error", _lastError);
        recordEvent("reset failed " + code.format("%d"));
    }

    function recordEvent(message) {
        if (_debugEvents == null) {
            _debugEvents = [];
        }

        var lastIndex = _debugEvents.size() - 1;
        if (lastIndex >= 0 && _debugEvents[lastIndex] == message) {
            return;
        }

        _debugEvents.add(message);

        // Trim to the last 3 entries in a single pass to avoid repeated
        // expensive `remove(0)` operations which can be O(n) and cause
        // long-running execution if the array has grown large.
        var maxEntries = 3;
        var size = _debugEvents.size();
        if (size > maxEntries) {
            var start = size - maxEntries;
            var trimmed = [];
            for (var i = start; i < size; i += 1) {
                trimmed.add(_debugEvents[i]);
            }
            _debugEvents = trimmed;
        }
    }

    function jsonValue(payload, symbolKey, stringKey) {
        if (payload == null || !(payload instanceof Lang.Dictionary)) {
            return null;
        }

        var value = payload[symbolKey];
        if (value != null) {
            return value;
        }

        return payload[stringKey];
    }

    function jsonStringValue(payload, symbolKey, stringKey) {
        var value = jsonValue(payload, symbolKey, stringKey);
        if (value != null && (value instanceof Lang.String)) {
            return value;
        }

        return null;
    }

    function stringEquals(value, expected) {
        return value != null && value.equals(expected);
    }

    function loadOrCreateInstallId() {
        var stored = loadStoredValue(STORAGE_KEYS[:install_id]);
        if (stored != null && stored.length() >= 8) {
            return stored;
        }

        var now = Time.now().value();
        var randomPart = Math.rand();
        var generated = "install-" + now.format("%d") + "-" + randomPart.format("%d");
        storeValue(STORAGE_KEYS[:install_id], generated);
        return generated;
    }

    function loadStoredValue(key) {
        var app = Application.getApp();
        var value = app.getProperty(key);
        if (value == null) {
            return null;
        }
        return value;
    }

    function storeValue(key, value) {
        var app = Application.getApp();
        if (value == null) {
            app.deleteProperty(key);
            return;
        }

        app.setProperty(key, value);
    }

    function loadAuthState() {
        _pairingCode = loadStoredValue(authStorageKeyFor(STORAGE_KEYS[:pairing_code_prefix]));
        _refreshToken = loadStoredValue(authStorageKeyFor(STORAGE_KEYS[:refresh_token_prefix]));
        _accessToken = loadStoredValue(authStorageKeyFor(STORAGE_KEYS[:access_token_prefix]));
    }

    function storePairingCode(value) {
        storeValue(authStorageKeyFor(STORAGE_KEYS[:pairing_code_prefix]), value);
    }

    function storeRefreshToken(value) {
        storeValue(authStorageKeyFor(STORAGE_KEYS[:refresh_token_prefix]), value);
    }

    function storeAccessToken(value) {
        storeValue(authStorageKeyFor(STORAGE_KEYS[:access_token_prefix]), value);
    }

    function authStorageKeyFor(prefix) {
        return prefix + authStorageSuffixFor(_apiBaseUrl);
    }

    function authStorageSuffixFor(apiBaseUrl) {
        // Use a stable, short suffix for storage keys so properties
        // are stored and retrieved consistently across runs.  Using
        // the full URL as a suffix can include characters that make
        // property keys brittle in the simulator environment.
        if (apiBaseUrl == API_BASE_URL_LOCAL) {
            return "local";
        }

        return "prod";
    }

    function loadStringResource(resourceId) {
        try {
            var value = Application.loadResource(resourceId);
            if (value != null && (value instanceof Lang.String)) {
                return value;
            }
        } catch (ex) {
        }

        return null;
    }

    function defaultAccountUrlFor(apiBaseUrl) {
        if (stringEquals(apiBaseUrl, API_BASE_URL_LOCAL)) {
            return ACCOUNT_URL_LOCAL;
        }

        return ACCOUNT_URL_PROD;
    }

    function defaultAccountLabelFor(apiBaseUrl) {
        if (stringEquals(apiBaseUrl, API_BASE_URL_LOCAL)) {
            return ACCOUNT_LABEL_LOCAL;
        }

        return ACCOUNT_LABEL_PROD;
    }

    function apiLabelFor(apiBaseUrl) {
        if (stringEquals(apiBaseUrl, API_BASE_URL_LOCAL)) {
            return "localhost:3000";
        }

        if (stringEquals(apiBaseUrl, API_BASE_URL_PROD)) {
            return "bike.nibelheim.dev";
        }

        return apiBaseUrl;
    }

    function shouldFallback(code as Lang.Number) {
        return _allowProductionFallback
            && stringEquals(_apiBaseUrl, API_BASE_URL_LOCAL)
            && (code <= 0 || code == 404 || code >= 500);
    }

    function switchToProductionApi() {
        _apiBaseUrl = API_BASE_URL_PROD;
        _accountUrl = ACCOUNT_URL_PROD;
        _accountLabel = ACCOUNT_LABEL_PROD;
        _verificationUrl = _accountUrl;
        storeValue(STORAGE_KEYS[:api_base_url], _apiBaseUrl);
        loadAuthState();
    }
}
