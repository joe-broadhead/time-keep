(function () {
  function renderMermaid() {
    if (!window.mermaid) {
      return;
    }

    window.mermaid.initialize({ startOnLoad: false, securityLevel: "strict" });
    const render = window.mermaid.run({ querySelector: ".mermaid" });
    if (render && typeof render.catch === "function") {
      render.catch(console.error);
    }
  }

  if (typeof document$ !== "undefined") {
    document$.subscribe(renderMermaid);
    return;
  }

  window.addEventListener("load", renderMermaid);
})();
