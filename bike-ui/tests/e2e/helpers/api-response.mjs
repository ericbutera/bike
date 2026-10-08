let bindingSequence = 0;

// Explicit API response fakes for error-state browser tests. Constructing the
// Response at fetch's boundary avoids Chromium's automatic failed-resource
// diagnostic for an intentionally unsuccessful HTTP response. All unexpected
// browser warnings/errors still reach the shared diagnostics fixture.
export async function fakeApiResponse(page, paths, respond) {
  const binding = `bikeApiResponse${bindingSequence++}`;
  await page.exposeBinding(binding, (_, pathname) => respond(pathname));
  await page.addInitScript(
    ({ binding, paths }) => {
      const fetch = window.fetch.bind(window);
      window.fetch = async (input, options) => {
        const address = input instanceof Request ? input.url : input.toString();
        const { pathname } = new URL(address, window.location.href);
        if (!paths.includes(pathname)) return fetch(input, options);
        const { status, json } = await window[binding](pathname);
        return new Response(JSON.stringify(json), {
          status,
          headers: { "content-type": "application/json" },
        });
      };
    },
    { binding, paths },
  );
}
