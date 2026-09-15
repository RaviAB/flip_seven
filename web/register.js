const localPreview = ["localhost", "127.0.0.1", "[::1]"].includes(location.hostname);
if ("serviceWorker" in navigator && (!localPreview || new URL(location.href).searchParams.has("pwa"))) {
  window.addEventListener("load", () => {
    navigator.serviceWorker.register("./sw.js", { scope: "./", updateViaCache: "none" })
      .catch(error => console.warn("Offline installation unavailable:", error));
  });
}
