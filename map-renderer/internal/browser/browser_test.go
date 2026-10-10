package browser

import (
	"context"
	"errors"
	"testing"

	"github.com/chromedp/chromedp"
	"github.com/stretchr/testify/require"
)

func TestBrowserStartupFailure(t *testing.T) {
	_, err := New(context.Background(), "/missing-browser-fixture", "http://localhost/")
	require.Error(t, err)
}

func TestConsoleWarningsErrorsAndExceptionsFailRendering(t *testing.T) {
	for _, kind := range []chromedp.ConsoleType{chromedp.ConsoleWarning, chromedp.ConsoleError, chromedp.ConsoleException, chromedp.ConsoleInfo} {
		var d diagnostics
		d.collect(func(yield func(chromedp.ConsoleMessage, error) bool) {
			yield(chromedp.ConsoleMessage{Type: kind, Text: "fixture"}, nil)
		})
		if kind == chromedp.ConsoleInfo {
			require.NoError(t, d.err())
		} else {
			require.ErrorContains(t, d.err(), "fixture")
		}
	}
	var d diagnostics
	failure := errors.New("console protocol fixture")
	d.collect(func(yield func(chromedp.ConsoleMessage, error) bool) { yield(chromedp.ConsoleMessage{}, failure) })
	require.ErrorIs(t, d.err(), failure)
	d = diagnostics{}
	d.collect(func(yield func(chromedp.ConsoleMessage, error) bool) {
		yield(chromedp.ConsoleMessage{}, context.Canceled)
	})
	require.NoError(t, d.err())
}
