import { Mic, Settings } from "lucide-react";
import type { StreamStatus } from "../types";

export interface HeaderProps {
  status: StreamStatus;
  isActive: boolean;
  isStreaming: boolean;
  onOpenSettings: () => void;
}

export function Header({
  status,
  isActive,
  isStreaming,
  onOpenSettings,
}: HeaderProps) {
  return (
    <header className="flex items-center justify-between px-6 py-4 border-b border-slate-800 bg-slate-900/60 backdrop-blur sticky top-0 z-20">
      <div className="flex items-center gap-3">
        <div className="p-2 rounded-lg bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 shadow-sm">
          <Mic className="w-5 h-5" />
        </div>
        <div>
          <div className="flex items-center gap-2">
            <h1 className="text-base font-semibold leading-none tracking-tight text-white">
              MicStream
            </h1>
            <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 font-medium">
              v0.1
            </span>
          </div>
          <p className="text-xs text-slate-400 mt-1">Ultra-Low Latency LAN Audio</p>
        </div>
      </div>

      <div className="flex items-center gap-3">
        {/* Status Badge */}
        <div className="flex items-center gap-2 px-3 py-1 rounded-full text-xs font-medium border bg-slate-900/90 border-slate-800 shadow-inner">
          {isActive ? (
            <>
              <span className="relative flex h-2 w-2">
                <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75" />
                <span className="relative inline-flex rounded-full h-2 w-2 bg-emerald-500" />
              </span>
              <span className="text-emerald-400 font-medium">
                {isStreaming ? "Streaming" : "Listening"}
              </span>
            </>
          ) : status === "Disconnected" ? (
            <>
              <span className="inline-flex rounded-full h-2 w-2 bg-amber-500" />
              <span className="text-amber-400 font-medium">Waiting for Host</span>
            </>
          ) : status === "Error" ? (
            <>
              <span className="inline-flex rounded-full h-2 w-2 bg-rose-500" />
              <span className="text-rose-400 font-medium">Error</span>
            </>
          ) : (
            <>
              <span className="inline-flex rounded-full h-2 w-2 bg-slate-500" />
              <span className="text-slate-400 font-medium">Ready</span>
            </>
          )}
        </div>

        {/* Settings Button */}
        <button
          type="button"
          onClick={onOpenSettings}
          className="p-2 rounded-lg text-slate-400 hover:text-slate-200 hover:bg-slate-800/80 transition-colors cursor-pointer"
          title="Audio & Network Settings"
          aria-label="Settings"
        >
          <Settings className="w-4 h-4" />
        </button>
      </div>
    </header>
  );
}
