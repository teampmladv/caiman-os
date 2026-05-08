import React, { useState } from 'react'
import { loadClusters, getActiveCluster, setActiveCluster, removeCluster } from './ClusterStore.js'
import ConnectClusterModal from './ConnectClusterModal.jsx'

const COLOR_HEX = { cyan:'#00d4ff', green:'#22c55e', amber:'#ffb800', rose:'#ff4466', violet:'#a855f7' }

export default function ClusterSidebarList({ onSelect }) {
  const [clusters, setClusters] = useState(loadClusters)
  const [active, setActive]     = useState(getActiveCluster)
  const [showModal, setShowModal] = useState(false)

  const hex = (c) => COLOR_HEX[c.color] || '#22c55e'

  const select = (c) => {
    setActiveCluster(c.id); setActive(c)
    window.dispatchEvent(new CustomEvent('caiman:cluster-changed', { detail: c }))
    onSelect?.(c)
  }

  const remove = (e, id) => {
    e.stopPropagation()
    removeCluster(id)
    const updated = loadClusters()
    setClusters(updated)
    if (active?.id === id) { setActive(null); setActiveCluster(null) }
  }

  const onConnected = (c) => {
    setClusters(loadClusters()); select(c); setShowModal(false)
  }

  return (
    <>
      {clusters.length === 0 && (
        <div style={{ fontSize:10, color:'#334155', padding:'4px 16px 8px', fontStyle:'italic' }}>No clusters</div>
      )}
      {clusters.map(c => {
        const isActive = active?.id === c.id
        const h = hex(c)
        return (
          <button key={c.id} onClick={() => select(c)}
            style={{ width:'100%', display:'flex', alignItems:'center', gap:8, padding:'6px 16px',
              background: isActive ? `${h}0d` : 'transparent', border:'none',
              borderLeft: isActive ? `2px solid ${h}` : '2px solid transparent',
              cursor:'pointer', transition:'all 0.15s', textAlign:'left' }}>
            <span style={{ width:7, height:7, borderRadius:'50%', background:h, flexShrink:0,
              boxShadow: isActive ? `0 0 6px ${h}` : 'none',
              animation: isActive ? 'pulse 2s infinite' : 'none' }} />
            <div style={{ flex:1, minWidth:0 }}>
              <div style={{ fontSize:11, color: isActive ? h : '#94a3b8',
                fontFamily:'IBM Plex Mono, monospace', overflow:'hidden',
                textOverflow:'ellipsis', whiteSpace:'nowrap' }}>
                {c.name}
              </div>
            </div>
            {isActive && <span style={{ fontSize:8, color:h, letterSpacing:'0.06em' }}>LIVE</span>}
            <button onClick={e => remove(e, c.id)}
              style={{ background:'none', border:'none', color:'#334155', cursor:'pointer',
                fontSize:12, padding:'0 2px', lineHeight:1, flexShrink:0,
                transition:'color 0.15s' }}
              onMouseEnter={e => e.target.style.color='#ff4466'}
              onMouseLeave={e => e.target.style.color='#334155'}>x</button>
          </button>
        )
      })}
      <button onClick={() => setShowModal(true)}
        style={{ width:'100%', display:'flex', alignItems:'center', gap:8, padding:'6px 16px 10px',
          background:'transparent', border:'none', borderLeft:'2px solid transparent',
          color:'#475569', cursor:'pointer', fontSize:11,
          fontFamily:'IBM Plex Mono, monospace', textAlign:'left' }}>
        <span style={{ fontSize:16, fontWeight:300 }}>+</span>
        <span>Connect cluster</span>
      </button>
      {showModal && <ConnectClusterModal onClose={() => setShowModal(false)} onConnected={onConnected} />}
    </>
  )
}
