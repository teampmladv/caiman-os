import { AppProvider, useApp } from './store.jsx'
import Dashboard from './views/Dashboard.jsx'
import VMs from './views/VMs.jsx'
import Topology from './views/Topology.jsx'
import Console from './views/Console.jsx'
import Snapshots from './views/Snapshots.jsx'
import DRS from './views/DRS.jsx'
import Billing from './views/Billing.jsx'
import Tenants from './views/Tenants.jsx'
import Alerts from './views/Alerts.jsx'
import APIKeys from './views/APIKeys.jsx'
import Backup from './views/Backup.jsx'
import GPU from './views/GPU.jsx'
import Storage from './views/Storage.jsx'
import Network from './views/Network.jsx'
import Logs from './views/Logs.jsx'
import Import from './views/Import.jsx'
import ClusterView from './views/ClusterView.jsx'
import ClusterSidebarList from './components/clusters/ClusterSidebarList.jsx'
import { getActiveCluster } from './components/clusters/ClusterStore.js'

const NAV = [
  { id: 'dashboard', label: 'Dashboard', group: 'overview',  icon: <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" width="18" height="18"><rect x="2" y="2" width="7" height="7" rx="1"/><rect x="11" y="2" width="7" height="7" rx="1"/><rect x="2" y="11" width="7" height="7" rx="1"/><rect x="11" y="11" width="7" height="7" rx="1"/></svg> },
  { id: 'vms',       label: 'Virtual Machines', group: 'compute', icon: <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" width="18" height="18"><rect x="1" y="5" width="18" height="10" rx="2"/><path d="M5 5V4M15 5V4"/><path d="M5 10h2M9 10h2"/></svg> },
  { id: 'topo',      label: 'Topology', group: 'compute', icon: <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" width="18" height="18"><circle cx="10" cy="4" r="2"/><circle cx="4" cy="16" r="2"/><circle cx="16" cy="16" r="2"/><path d="M10 6v4M10 10l-4.5 5M10 10l4.5 5"/></svg> },
  { id: 'console',   label: 'Console', group: 'compute', icon: <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" width="18" height="18"><rect x="1" y="3" width="18" height="14" rx="2"/><path d="M6 8l3 2-3 2M12 12h3"/></svg> },
  { id: 'snapshots', label: 'Snapshots', group: 'compute', icon: <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" width="18" height="18"><circle cx="10" cy="10" r="7"/><path d="M10 7v3l2 2"/><path d="M3.5 3.5l4 4M12.5 12.5l4 4"/></svg> },
  { id: 'storage',   label: 'Storage', group: 'infra', icon: <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" width="18" height="18"><ellipse cx="10" cy="6" rx="7" ry="2.5"/><path d="M3 6v8c0 1.38 3.13 2.5 7 2.5s7-1.12 7-2.5V6"/><path d="M3 10c0 1.38 3.13 2.5 7 2.5s7-1.12 7-2.5"/></svg> },
  { id: 'network',   label: 'Networking', group: 'infra', icon: <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" width="18" height="18"><rect x="1" y="3" width="6" height="4" rx="1"/><rect x="1" y="13" width="6" height="4" rx="1"/><rect x="13" y="8" width="6" height="4" rx="1"/><path d="M7 5h4v10H7M11 10h2"/></svg> },
  { id: 'gpu',       label: 'GPU Passthrough', group: 'infra', icon: <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" width="18" height="18"><rect x="1" y="5" width="18" height="10" rx="2"/><path d="M5 9h2v2H5zM9 9h2v2H9zM13 9h2v2h-2z"/></svg> },
  { id: 'drs',       label: 'DRS / Scheduler', group: 'policies', icon: <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" width="18" height="18"><circle cx="10" cy="10" r="7"/><path d="M10 6v4l3 1.5"/><path d="M7 3.5L10 6M13 3.5L10 6"/></svg> },
  { id: 'alerts',    label: 'Alerts', group: 'policies', icon: <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" width="18" height="18"><path d="M10 2L2 17h16L10 2z"/><path d="M10 8v4M10 14h0"/></svg> },
  { id: 'backup',    label: 'Backup / DR', group: 'policies', icon: <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" width="18" height="18"><path d="M10 2a8 8 0 100 16A8 8 0 0010 2z"/><path d="M10 6v4l-3 3"/><path d="M6 2.5A8 8 0 012.5 6"/></svg> },
  { id: 'billing',   label: 'Billing', group: 'admin', icon: <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" width="18" height="18"><rect x="2" y="4" width="16" height="12" rx="2"/><path d="M2 8h16M6 12h2M10 12h4"/></svg> },
  { id: 'tenants',   label: 'Tenants / Roles', group: 'admin', icon: <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" width="18" height="18"><circle cx="7" cy="6" r="3"/><circle cx="14" cy="8" r="2.5"/><path d="M1 17c0-3.31 2.69-6 6-6s6 2.69 6 6"/><path d="M16 13c1.66 0 3 1.34 3 3"/></svg> },
  { id: 'apikeys',   label: 'API Keys', group: 'admin', icon: <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" width="18" height="18"><circle cx="7.5" cy="10" r="4.5"/><path d="M12 7.5l6 6M16 7.5l2 2"/></svg> },
  { id: 'import',    label: 'Import & Migrate', group: 'admin', icon: <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" width="18" height="18"><path d="M10 2v12M6 10l4 4 4-4"/><path d="M3 16h14"/></svg> },
  { id: 'logs',      label: 'Live Logs', group: 'admin', icon: <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" width="18" height="18"><path d="M4 5h12M4 9h8M4 13h6M4 17h10"/></svg> },
]

const GROUPS = {
  overview: 'OVERVIEW',
  compute:  'COMPUTE',
  infra:    'INFRASTRUCTURE',
  policies: 'POLICIES',
  admin:    'ADMINISTRATION',
}

const VIEWS = { cluster: ClusterView, import: Import, dashboard: Dashboard, vms: VMs, topo: Topology, console: Console, snapshots: Snapshots, storage: Storage, network: Network, gpu: GPU, drs: DRS, alerts: Alerts, backup: Backup, billing: Billing, tenants: Tenants, apikeys: APIKeys, logs: Logs }

function Layout() {
  const { view, setView, toast, vms, alerts } = useApp()
  const ActiveView = VIEWS[view] || Dashboard
  const firedAlerts = alerts.filter(a => a.active && a.fired > 0).length
  const runningVMs = vms.filter(v => v.status === 'RUNNING').length

  const groups = {}
  NAV.forEach(item => {
    if (!groups[item.group]) groups[item.group] = []
    groups[item.group].push(item)
  })

  return (
    <div style={{ display: 'grid', gridTemplateColumns: '200px 1fr', height: '100vh', background: '#0a0e1a' }}>
      {/* Sidebar */}
      <aside style={{ background: '#070b14', borderRight: '1px solid #1e2a3a', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
        {/* Logo */}
        <div style={{ padding: '16px 16px 12px', borderBottom: '1px solid #1e2a3a', display: 'flex', alignItems: 'center', gap: 10 }}>
          <svg viewBox="0 0 24 24" fill="none" width="22" height="22">
            <ellipse cx="12" cy="12" rx="10" ry="7" stroke="#22c55e" strokeWidth="1.5"/>
            <circle cx="8" cy="10.5" r="2" fill="#22c55e"/>
            <circle cx="7.5" cy="10" r="0.8" fill="#070b14"/>
            <path d="M12 19C12 19 19 17 21 12" stroke="#22c55e" strokeWidth="1.2" strokeLinecap="round"/>
            <path d="M3 12C5 16 8 18 12 18" stroke="#22c55e" strokeWidth="1.2" strokeLinecap="round"/>
          </svg>
          <div>
            <div style={{ fontFamily: 'Syne, sans-serif', fontWeight: 800, fontSize: 13, color: '#22c55e', letterSpacing: '0.05em' }}>CAIMÁN UI</div>
            <div style={{ fontSize: 9, color: '#475569', letterSpacing: '0.08em' }}>v1.0.3 · {runningVMs} VMs running</div>
          </div>
        </div>

        {/* Nav */}
        <nav style={{ flex: 1, overflowY: 'auto', padding: '8px 0' }}>
          {Object.entries(groups).map(([group, items]) => (
            <div key={group}>
              {group === 'compute' && (
                <div style={{ borderTop:'1px solid #1e2a3a', borderBottom:'1px solid #1e2a3a', marginBottom:4 }}>
                  <div style={{ fontSize:9, color:'#334155', letterSpacing:'0.14em', padding:'10px 16px 4px', fontWeight:500 }}>CLUSTERS</div>
                  <ClusterSidebarList onSelect={() => setView('cluster')} />
                </div>
              )}
              <div style={{ fontSize: 9, color: '#334155', letterSpacing: '0.14em', padding: '10px 16px 4px', fontWeight: 500 }}>{GROUPS[group]}</div>
              {items.map(item => (
                <button key={item.id} onClick={() => setView(item.id)}
                  style={{ width: '100%', display: 'flex', alignItems: 'center', gap: 10, padding: '7px 16px', background: view === item.id ? 'rgba(34,197,94,0.08)' : 'transparent', border: 'none', borderLeft: view === item.id ? '2px solid #22c55e' : '2px solid transparent', color: view === item.id ? '#22c55e' : '#cbd5e1', cursor: 'pointer', fontSize: 11, fontFamily: 'IBM Plex Mono, monospace', transition: 'all 0.15s', textAlign: 'left' }}>
                  <span style={{ color: view === item.id ? '#22c55e' : '#94a3b8', flexShrink: 0 }}>{item.icon}</span>
                  <span style={{ flex: 1 }}>{item.label}</span>
                  {item.id === 'alerts' && firedAlerts > 0 && <span style={{ background: '#dc2626', color: '#fff', fontSize: 9, padding: '1px 5px', borderRadius: 2 }}>{firedAlerts}</span>}
                </button>
              ))}
            </div>
          ))}
        </nav>
        {/* User */}
        <div style={{ padding: '12px 16px', borderTop: '1px solid #1e2a3a', display: 'flex', alignItems: 'center', gap: 8 }}>
          <div style={{ width: 28, height: 28, borderRadius: '50%', background: 'rgba(34,197,94,0.15)', border: '1px solid rgba(34,197,94,0.3)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 10, color: '#22c55e', fontWeight: 600 }}>OG</div>
          <div>
            <div style={{ fontSize: 11, color: '#e2e8f0' }}>Ogandi</div>
            <div style={{ fontSize: 9, color: '#475569' }}>Admin · Capablanca</div>
          </div>
        </div>
      </aside>

      {/* Main */}
      <main style={{ display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
        {/* Topbar */}
        <div style={{ height: 48, background: '#080c18', borderBottom: '1px solid #1e2a3a', display: 'flex', alignItems: 'center', padding: '0 20px', gap: 12, flexShrink: 0 }}>
          <div style={{ fontFamily: 'Syne, sans-serif', fontWeight: 800, fontSize: 14, color: '#e2e8f0', flex: 1 }}>
            {NAV.find(n => n.id === view)?.label || 'Dashboard'}
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 5, fontSize: 10, color: '#22c55e' }}>
            <span style={{ width: 5, height: 5, background: '#22c55e', borderRadius: '50%', display: 'inline-block', animation: 'pulse 2s infinite' }}></span>
            CLUSTER LIVE
          </div>
          <div style={{ fontSize: 10, color: '#475569', padding: '3px 8px', border: '1px solid #1e2a3a', fontFamily: 'IBM Plex Mono, monospace' }}>
            {getActiveCluster()?.name || 'no cluster'}
          </div>
        </div>

        {/* View */}
        <div style={{ flex: 1, overflow: 'hidden' }}>
          <ActiveView />
        </div>
      </main>

      {/* Toast */}
      {toast && (
        <div style={{ position: 'fixed', bottom: 20, right: 20, background: '#0d1220', border: '1px solid #2d3f55', padding: '10px 16px', fontSize: 12, color: toast.type === 'info' ? '#60a5fa' : '#22c55e', zIndex: 1000, borderRadius: 3, animation: 'fadeIn 0.2s ease' }}>
          {toast.msg}
        </div>
      )}

      <style>{`
        @keyframes pulse { 0%,100%{opacity:1} 50%{opacity:0.2} }
        @keyframes fadeIn { from{opacity:0;transform:translateY(4px)} to{opacity:1;transform:translateY(0)} }
        button:hover { opacity: 0.85; }
      `}</style>
    </div>
  )
}

export default function App() {
  return (
    <AppProvider>
      <Layout />
    </AppProvider>
  )
}
