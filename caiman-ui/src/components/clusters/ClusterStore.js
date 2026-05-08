const STORAGE_KEY = 'caiman_clusters';
const ACTIVE_KEY  = 'caiman_active_cluster';

export function loadClusters() {
  try { return JSON.parse(localStorage.getItem(STORAGE_KEY) || '[]'); }
  catch { return []; }
}
export function saveClusters(clusters) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(clusters));
}
export function addCluster({ name, url, token, color }) {
  const clusters = loadClusters();
  const id = crypto.randomUUID();
  const entry = { id, name, url: url.replace(/\/$/, ''), token, color, addedAt: new Date().toISOString() };
  saveClusters([...clusters, entry]);
  return entry;
}
export function removeCluster(id) {
  saveClusters(loadClusters().filter(c => c.id !== id));
  if (getActiveClusterId() === id) setActiveCluster(null);
}
export function getActiveClusterId() { return localStorage.getItem(ACTIVE_KEY); }
export function setActiveCluster(id) {
  if (id) localStorage.setItem(ACTIVE_KEY, id);
  else localStorage.removeItem(ACTIVE_KEY);
}
export function getActiveCluster() {
  const id = getActiveClusterId();
  return loadClusters().find(c => c.id === id) || null;
}
export async function probeCluster({ url, token }) {
  const res = await fetch(`${url}/health`, {
    headers: { Authorization: `Bearer ${token}` },
    signal: AbortSignal.timeout(5000),
    mode: 'cors',
  });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.json();
}
export function decodeTokenInfo(token) {
  try {
    const raw = token.startsWith('caim_') ? token.slice(5) : token;
    const payload = JSON.parse(atob(raw.split('.')[1]));
    return {
      name: payload.sub, role: payload.role, cluster: payload.cluster,
      expiresAt: payload.exp ? new Date(payload.exp * 1000) : null,
      isExpired: payload.exp ? Date.now() > payload.exp * 1000 : false,
    };
  } catch { return null; }
}
