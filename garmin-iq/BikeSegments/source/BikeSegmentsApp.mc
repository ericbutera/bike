using Toybox.Application;
using Toybox.WatchUi;

class BikeSegmentsApp extends Application.AppBase {

    function initialize() {
        AppBase.initialize();
    }

    function onStart(state) {
    }

    function getInitialView() {
        var view = new BikeSegmentsView();
        var delegate = new BikeSegmentsDelegate(view);
        return [view, delegate];
    }
}

function getApp() {
    return Application.getApp();
}
