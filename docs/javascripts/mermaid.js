document$.subscribe(function () {
  if (window.mermaid) {
    mermaid.initialize({ startOnLoad: true, theme: document.body.dataset.mdColorScheme === 'slate' ? 'dark' : 'default' });
  }
});
