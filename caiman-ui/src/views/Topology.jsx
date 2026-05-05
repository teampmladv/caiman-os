import { useEffect, useRef } from 'react'
import { useApp } from '../store.jsx'
import { C } from '../components/UI.jsx'

export default function Topology() {
  const { nodes, vms } = useApp()
  const svgRef = useRef()

  useEffect(() => {
    const svg = svgRef.current
    if (!svg) return
    const W = svg.clientWidth || 600
    const H = svg.clientHeight || 500
    const cx = W / 2, cy = H / 2
    const angles = [270, 30, 150]
    const nodePos = nodes.map((n, i) => {
      const a = angles[i] * Math.PI / 180
      return { x: cx + 140 * Math.cos(a), y: cy + 130 * Math.sin(a), n }
    })

    let s = `<rect width="${W}" height="${H}" fill="#080c18"/>`
    s += `<rect x="${cx-30}" y="${cy-20}" width="60" height="40" rx="3" fill="#0d1220" stroke="#22c55e" stroke-width="1.5"/>`
    s += `<text x="${cx}" y="${cy-6}" text-anchor="middle" fill="#22c55e" font-family="IBM Plex Mono" font-size="9" letter-spacing="1">CLUSTER</text>`
    s += `<text x="${cx}" y="${cy+8}" text-anchor="middle" fill="#64748b" font-family="IBM Plex Mono" font-size="8">core</text>`

    nodePos.forEach((np) => {
      const n = np.n
      const c = n.cpu > 80 ? '#f87171' : n.cpu > 60 ? '#fbbf24' : '#22c55e'
      s += `<line x1="${cx}" y1="${cy}" x2="${np.x}" y2="${np.y}" stroke="#1e2a3a" stroke-width="1.5"/>`
      s += `<rect x="${np.x-44}" y="${np.y-26}" width="88" height="52" rx="3" fill="#111827" stroke="${c}" stroke-width="1"/>`
      s += `<text x="${np.x}" y="${np.y-10}" text-anchor="middle" fill="#e2e8f0" font-family="IBM Plex Mono" font-size="10" font-weight="500">${n.name.replace('caiman-', '')}</text>`
      s += `<text x="${np.x}" y="${np.y+4}" text-anchor="middle" fill="${c}" font-family="IBM Plex Mono" font-size="9">CPU ${Math.round(n.cpu)}%</text>`
      s += `<text x="${np.x}" y="${np.y+17}" text-anchor="middle" fill="#64748b" font-family="IBM Plex Mono" font-size="9">${n.cores}c · ${n.ram}GiB · ${n.vms}VMs</text>`

      const nodeVMs = vms.filter(v => v.node === n.id && v.status === 'RUNNING')
      nodeVMs.forEach((vm, vi) => {
        const vangle = (angles[nodes.indexOf(n)] + (vi - nodeVMs.length / 2 + 0.5) * 35) * Math.PI / 180
        const vx = np.x + 110 * Math.cos(vangle)
        const vy = np.y + 110 * Math.sin(vangle)
        const vc = vm.cpu > 80 ? '#f87171' : vm.cpu > 60 ? '#fbbf24' : '#22c55e'
        s += `<line x1="${np.x}" y1="${np.y}" x2="${vx}" y2="${vy}" stroke="#1e2a3a" stroke-width="1" stroke-dasharray="3,3"/>`
        s += `<rect x="${vx-28}" y="${vy-16}" width="56" height="32" rx="2" fill="#0d1220" stroke="${C.brd}"/>`
        s += `<text x="${vx}" y="${vy-3}" text-anchor="middle" fill="#94a3b8" font-family="IBM Plex Mono" font-size="8">${vm.name.substring(0, 10)}</text>`
        s += `<text x="${vx}" y="${vy+9}" text-anchor="middle" fill="${vc}" font-family="IBM Plex Mono" font-size="8">${Math.round(vm.cpu)}% · ${vm.mem}M</text>`
      })
    })
    svg.innerHTML = s
  }, [nodes, vms])

  return (
    <div style={{ height: '100%', background: '#080c18', display: 'flex', flexDirection: 'column' }}>
      <div style={{ padding: '10px 16px', borderBottom: `1px solid ${C.brd}`, fontSize: 10, color: C.dim }}>
        Cluster topology · Drag nodes in VM view to perform live migrations
      </div>
      <svg ref={svgRef} style={{ flex: 1, width: '100%' }} />
    </div>
  )
}
