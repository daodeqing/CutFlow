(() => {
  const value = decodeURIComponent(location.hash.slice(1));
  const match = value.match(/^cutflow-upload=([^\.]+)\.([^\.]+)\.(\d+)$/);
  if (!match) return;
  const [jobId, token, port] = match.slice(1);
  chrome.runtime.sendMessage({
    type: 'cutflow-upload',
    jobId,
    token,
    port: Number(port),
    page: location.hostname === 'studio.youtube.com' ? 'studio' : 'channel',
  });
})();
