export const C = {
  bg: '#0a0e1a', bg2: '#0d1220', bg3: '#111827',
  brd: '#1e2a3a', brd2: '#2d3f55',
  txt: '#e2e8f0', dim: '#64748b', muted: '#334155',
  g: '#22c55e', g2: '#16a34a',
  red: '#f87171', amb: '#fbbf24', blu: '#60a5fa', pur: '#a78bfa',
}

export function Card({ children, style = {} }) {
  return <div style={{ background: C.bg2, border: `1px solid ${C.brd}`, padding: '14px', ...style }}>{children}</div>
}

export function Badge({ children, color = C.g, style = {} }) {
  return (
    <span style={{ fontSize: 9, padding: '2px 6px', background: color + '18', color, border: `1px solid ${color}30`, letterSpacing: '0.08em', ...style }}>
      {children}
    </span>
  )
}

export function StatusDot({ status }) {
  const colors = { RUNNING: C.g, BOOTING: C.amb, STOPPED: C.dim, HEALTHY: C.g, WARN: C.amb, CRIT: C.red }
  const color = colors[status] || C.dim
  return <span style={{ width: 6, height: 6, borderRadius: '50%', background: color, display: 'inline-block', marginRight: 5, animation: (status === 'RUNNING' || status === 'HEALTHY') ? 'pulse 2s infinite' : 'none' }} />
}

export function Bar({ pct, color }) {
  const c = pct > 85 ? C.red : pct > 65 ? C.amb : color || C.g
  return (
    <div style={{ height: 2, background: C.brd, borderRadius: 1 }}>
      <div style={{ height: '100%', width: `${Math.min(pct, 100)}%`, background: c, borderRadius: 1, transition: 'width 0.8s ease' }} />
    </div>
  )
}

export function Btn({ children, onClick, primary, danger, small, disabled, style = {} }) {
  const bg = primary ? C.g2 : 'transparent'
  const color = primary ? '#fff' : danger ? C.red : C.dim
  const border = primary ? C.g2 : danger ? `${C.red}40` : C.brd
  return (
    <button onClick={onClick} disabled={disabled} style={{
      background: bg, color, border: `1px solid ${border}`,
      padding: small ? '3px 8px' : '6px 14px',
      fontFamily: 'IBM Plex Mono, monospace', fontSize: small ? 10 : 11,
      letterSpacing: '0.06em', cursor: disabled ? 'not-allowed' : 'pointer',
      opacity: disabled ? 0.5 : 1, transition: 'all 0.15s',
      ...style
    }}>{children}</button>
  )
}

export function SectionHeader({ title, action }) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 12 }}>
      <div style={{ fontSize: 10, color: C.dim, letterSpacing: '0.12em', textTransform: 'uppercase' }}>{title}</div>
      {action}
    </div>
  )
}

export function MetricCard({ label, value, sub, color = C.g }) {
  return (
    <Card>
      <div style={{ fontFamily: 'Syne, sans-serif', fontWeight: 800, fontSize: 28, color, lineHeight: 1, marginBottom: 4 }}>{value}</div>
      <div style={{ fontSize: 10, color: C.dim, letterSpacing: '0.08em', textTransform: 'uppercase' }}>{label}</div>
      {sub && <div style={{ fontSize: 10, color: C.muted, marginTop: 2 }}>{sub}</div>}
    </Card>
  )
}

export function Scroller({ children, style = {} }) {
  return <div style={{ flex: 1, overflowY: 'auto', padding: 16, ...style }}>{children}</div>
}

export function Table({ headers, rows, style = {} }) {
  return (
    <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 11, tableLayout: 'fixed', ...style }}>
      <thead>
        <tr>{headers.map((h, i) => <th key={i} style={{ padding: '8px 12px', textAlign: 'left', fontSize: 9, letterSpacing: '0.12em', textTransform: 'uppercase', color: C.dim, borderBottom: `1px solid ${C.brd}`, background: C.bg3, fontWeight: 400, width: h.width }}>{h.label}</th>)}</tr>
      </thead>
      <tbody>
        {rows.map((row, i) => (
          <tr key={i} style={{ borderBottom: `1px solid ${C.brd}` }}>
            {row.map((cell, j) => <td key={j} style={{ padding: '9px 12px', color: C.txt }}>{cell}</td>)}
          </tr>
        ))}
      </tbody>
    </table>
  )
}
