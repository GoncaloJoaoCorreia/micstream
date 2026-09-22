import React, { useState } from "react";
import { AlertTriangle, ExternalLink, RotateCw, Terminal } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { VirtualDriverStatus } from "../types";

export interface DriverSetupCardProps {
  status: VirtualDriverStatus | null;
  onRefresh?: () => void;
  isRefreshing?: boolean;
}

export const DriverSetupCard: React.FC<DriverSetupCardProps> = ({
  status,
  onRefresh,
  isRefreshing = false,
}) => {
  const [openError, setOpenError] = useState<string | null>(null);

  // If driver status is unknown or driver is already detected, nothing to show
  if (!status || status.detected) {
    return null;
  }

  const handleOpenLink = async () => {
    if (!status.install_url) return;
    try {
      setOpenError(null);
      await openUrl(status.install_url);
    } catch {
      try {
        window.open(status.install_url, "_blank");
      } catch (e) {
        setOpenError(e instanceof Error ? e.message : String(e));
      }
    }
  };

  return (
    <div className="p-4 rounded-xl bg-amber-500/10 border border-amber-500/30 text-slate-100 flex flex-col gap-3 backdrop-blur-sm animate-in fade-in duration-200">
      {/* Top Header */}
      <div className="flex items-start gap-3">
        <div className="p-2 rounded-lg bg-amber-500/20 text-amber-400 border border-amber-500/30 shrink-0 mt-0.5">
          <AlertTriangle className="w-5 h-5" />
        </div>
        <div className="flex-1">
          <h3 className="text-sm font-semibold text-amber-200 leading-snug">
            Virtual Audio Driver Required
          </h3>
          <p className="text-xs text-slate-300 mt-1 leading-relaxed">
            Host mode streams audio directly into a virtual microphone cable so Discord, OBS, or in-game voice chat can capture it. No virtual cable was detected on your system.
          </p>
        </div>
      </div>

      {/* Recommended Driver & Setup Instructions */}
      <div className="p-3 rounded-lg bg-slate-950/70 border border-amber-500/20 flex flex-col gap-2">
        <div className="flex items-center justify-between text-xs">
          <span className="text-slate-400">Recommended driver:</span>
          <span className="font-semibold text-amber-300">{status.driver_name}</span>
        </div>

        {status.instructions && (
          <div className="flex items-start gap-2 pt-2 border-t border-slate-800/80 text-xs text-slate-300">
            <Terminal className="w-3.5 h-3.5 text-slate-400 shrink-0 mt-0.5" />
            <span className="font-mono text-[11px] text-slate-300 break-all">
              {status.instructions}
            </span>
          </div>
        )}
      </div>

      {openError && (
        <p className="text-xs text-rose-400">
          Failed to open browser automatically: {openError}. Please visit: {status.install_url}
        </p>
      )}

      {/* Action Buttons */}
      <div className="flex items-center gap-2 pt-1">
        {status.install_url && (
          <button
            type="button"
            onClick={handleOpenLink}
            className="flex-1 flex items-center justify-center gap-1.5 py-2 px-3 rounded-lg bg-amber-500 hover:bg-amber-400 text-slate-950 font-semibold text-xs transition shadow-md shadow-amber-950"
          >
            <span>Download {status.driver_name}</span>
            <ExternalLink className="w-3.5 h-3.5" />
          </button>
        )}

        {onRefresh && (
          <button
            type="button"
            onClick={onRefresh}
            disabled={isRefreshing}
            className="flex items-center justify-center gap-1.5 py-2 px-3 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-200 text-xs font-medium border border-slate-700 transition disabled:opacity-50"
            title="Check if driver is installed"
          >
            <RotateCw className={`w-3.5 h-3.5 ${isRefreshing ? "animate-spin text-amber-400" : ""}`} />
            <span>Check Again</span>
          </button>
        )}
      </div>
    </div>
  );
};
