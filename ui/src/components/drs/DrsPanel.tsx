import React from 'react'
import { clsx } from 'clsx'
import type { DrsRecommendation, DrsMode } from '../../types'

export function DrsPanel({ recs, sigma, mode }: {
  recs: DrsRecommendation[]; sigma: number; mode: DrsMode
}) {
  return (
    <div className="panel">
      <div className="panel-hd">
        <span className="panel-title">DRS recommendations</span>
        <span className={clsx('text-[9px]', sigma > 0.10 ? 'text-caiman-amber' : 'text-caiman-green')}>
          σ={sigma.toFixed(3)}
        </span>
      </div>
      <div className="panel-body">
        {recs.slice(0, 3).map((r, i) => (
          <div key={i} className="flex items-center gap-2 py-1.5 border-b border-caiman-bg last:border-b-0">
            <span className="text-[10px] text-caiman-bright font-mono min-w-[28px] text-right">
              {r.score.toFixed(2)}
            </span>
            <div className="flex-1 min-w-0">
              <div className="text-[10px] text-[#e8f5e9] truncate">{r.vmName}</div>
              <div className="text-[8px] text-caiman-blue">{r.fromNode} → {r.toNode}</div>
            </div>
            <button className="text-[8px] px-1.5 py-0.5 border border-[#1565c0]
                               rounded bg-[#0a1f2e] text-caiman-blue hover:bg-[#0d2e3e]
                               transition-colors cursor-pointer">
              RUN
            </button>
          </div>
        ))}
        {recs.length === 0 && (
          <div className="text-[9px] text-caiman-dim py-2 text-center">Cluster balanced ✓</div>
        )}
        <div className="flex justify-between items-center mt-2 pt-2 border-t border-caiman-border">
          <span className="text-[8px] text-caiman-dim">{mode}</span>
          <button className="text-[8px] text-caiman-bright hover:underline cursor-pointer">
            Execute all ↗
          </button>
        </div>
      </div>
    </div>
  )
}
