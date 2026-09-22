import React from "react";
import { Volume2, VolumeX } from "lucide-react";

export interface VolumeMeterProps {
  level: number; // 0.0 to 1.0 (RMS)
  peak?: number; // 0.0 to 1.0 (Peak hold)
  label?: string;
  isActive?: boolean;
}

/**
 * Converts a linear RMS/peak value (0.0 - 1.0) to decibels (-50 dB floor to 0 dB ceiling)
 * and maps it to a percentage (0% - 100%).
 */
function levelToPercent(val: number): { percent: number; db: number; isSilent: boolean } {
  if (val <= 0.0005) {
    return { percent: 0, db: -Infinity, isSilent: true };
  }
  const db = 20 * Math.log10(val);
  const clampedDb = Math.max(-50, Math.min(0, db));
  // -50dB -> 0%, 0dB -> 100%
  const percent = Math.max(0, Math.min(100, ((clampedDb + 50) / 50) * 100));
  return { percent, db, isSilent: false };
}

export const VolumeMeter: React.FC<VolumeMeterProps> = ({
  level,
  peak = 0,
  label = "Audio Level",
  isActive = true,
}) => {
  const current = levelToPercent(isActive ? level : 0);
  const peakData = levelToPercent(isActive ? Math.max(level, peak) : 0);

  const isClipping = current.db > -1.5;

  return (
    <div className="flex flex-col gap-2 p-4 rounded-xl bg-slate-900/70 border border-slate-800 backdrop-blur-sm select-none">
      {/* Top Header: Label, Live dB Readout, and Clip Indicator */}
      <div className="flex items-center justify-between text-xs">
        <div className="flex items-center gap-2">
          {current.isSilent ? (
            <VolumeX className="w-4 h-4 text-slate-500" />
          ) : (
            <Volume2 className="w-4 h-4 text-emerald-400" />
          )}
          <span className="font-medium text-slate-300">{label}</span>
        </div>

        <div className="flex items-center gap-2">
          {/* Clipping Indicator */}
          {isClipping && (
            <span className="px-1.5 py-0.2 rounded text-[10px] font-bold bg-rose-500/20 text-rose-400 border border-rose-500/30 animate-pulse">
              CLIP
            </span>
          )}

          {/* Numeric Decibel Readout */}
          <span className="font-mono text-slate-400 text-xs">
            {current.isSilent ? "-∞ dB" : `${current.db.toFixed(1)} dB`}
          </span>
        </div>
      </div>

      {/* Meter Bar Container */}
      <div className="relative w-full h-3.5 rounded-full bg-slate-950 border border-slate-800/90 overflow-hidden shadow-inner">
        {/* Color Gradient Track (Filled according to level) */}
        <div
          className="h-full rounded-full bg-gradient-to-r from-emerald-500 via-amber-400 to-rose-500 transition-[width] duration-75 ease-out"
          style={{ width: `${current.percent}%` }}
        />

        {/* Peak Hold Marker */}
        {peakData.percent > 1 && (
          <div
            className="absolute top-0 bottom-0 w-1 bg-white/90 rounded-full shadow-md pointer-events-none transition-[left] duration-75 ease-out"
            style={{ left: `calc(${peakData.percent}% - 2px)` }}
          />
        )}
      </div>

      {/* Decibel Scale Markings */}
      <div className="flex justify-between px-0.5 text-[10px] text-slate-500 font-mono tracking-tighter">
        <span>-50</span>
        <span>-30</span>
        <span>-20</span>
        <span>-12</span>
        <span>-6</span>
        <span className="text-rose-400">0 dB</span>
      </div>
    </div>
  );
};
