import { useState } from "react";
import {
  Wifi,
  RefreshCw,
  Server,
  Play,
  Clock,
  X,
  ChevronDown,
  ChevronUp,
  Search,
  Zap,
} from "lucide-react";
import type { DiscoveredHost, RecentHost } from "../types";

export interface HostDiscoveryListProps {
  discoveredHosts: DiscoveredHost[];
  recentHosts: RecentHost[];
  isDiscovering: boolean;
  manualIp: string;
  manualPort: number;
  onManualIpChange: (ip: string) => void;
  onManualPortChange: (port: number) => void;
  onConnect: (ip: string, port: number, hostName?: string) => void;
  onRemoveRecentHost: (ip: string, port: number) => void;
  onRefreshDiscovery: () => void;
  disabled?: boolean;
  isConnecting?: boolean;
}

export function HostDiscoveryList({
  discoveredHosts,
  recentHosts,
  isDiscovering,
  manualIp,
  manualPort,
  onManualIpChange,
  onManualPortChange,
  onConnect,
  onRemoveRecentHost,
  onRefreshDiscovery,
  disabled = false,
  isConnecting = false,
}: HostDiscoveryListProps) {
  const [showManualForm, setShowManualForm] = useState<boolean>(true);

  const getPrimaryIp = (host: DiscoveredHost): string => {
    if (host.ip_addresses && host.ip_addresses.length > 0) {
      // Find IPv4 first
      const ipv4 = host.ip_addresses.find((ip) => !ip.includes(":"));
      return ipv4 ?? host.ip_addresses[0];
    }
    return "127.0.0.1";
  };

  const isManualFormValid =
    manualIp.trim().length > 0 && manualPort >= 1024 && manualPort <= 65535;

  return (
    <div className="flex flex-col gap-4">
      {/* 1. Discovered Hosts (mDNS) */}
      <div className="p-4 rounded-xl bg-slate-900/70 border border-slate-800 flex flex-col gap-3 shadow-sm">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div className="p-1 rounded bg-emerald-500/10 text-emerald-400">
              <Wifi className="w-4 h-4" />
            </div>
            <div>
              <span className="text-xs font-semibold text-slate-200">
                Discovered Hosts (LAN)
              </span>
              <span className="ml-2 text-[10px] font-mono px-1.5 py-0.2 rounded bg-slate-800 text-slate-400">
                mDNS
              </span>
            </div>
          </div>

          <div className="flex items-center gap-2">
            {isDiscovering && (
              <span className="flex items-center gap-1.5 text-[11px] text-emerald-400">
                <span className="relative flex h-2 w-2">
                  <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75" />
                  <span className="relative inline-flex rounded-full h-2 w-2 bg-emerald-500" />
                </span>
                Scanning
              </span>
            )}
            <button
              type="button"
              disabled={disabled || isDiscovering}
              onClick={onRefreshDiscovery}
              className="p-1.5 rounded-lg text-slate-400 hover:text-slate-200 hover:bg-slate-800 transition disabled:opacity-40 cursor-pointer"
              title="Rescan local network"
            >
              <RefreshCw
                className={`w-3.5 h-3.5 ${isDiscovering ? "animate-spin text-emerald-400" : ""}`}
              />
            </button>
          </div>
        </div>

        {/* Discovered Hosts List */}
        {discoveredHosts.length > 0 ? (
          <div className="flex flex-col gap-2">
            {discoveredHosts.map((host) => {
              const ip = getPrimaryIp(host);
              return (
                <div
                  key={`${host.host_name}-${ip}-${host.port}`}
                  className="flex items-center justify-between p-3 rounded-lg bg-slate-950/70 border border-slate-800/80 hover:border-slate-700 transition group"
                >
                  <div className="flex items-center gap-3 min-w-0">
                    <div className="p-2 rounded-md bg-slate-900 border border-slate-800 text-slate-300 group-hover:text-emerald-400 transition shrink-0">
                      <Server className="w-4 h-4" />
                    </div>
                    <div className="min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-xs font-medium text-slate-100 truncate">
                          {host.host_name}
                        </span>
                        <span className="text-[10px] font-mono px-1 py-0.2 rounded bg-emerald-950/60 border border-emerald-800/40 text-emerald-400">
                          Ready
                        </span>
                      </div>
                      <div className="flex items-center gap-1 text-[11px] text-slate-400 font-mono mt-0.5 truncate">
                        <span>{ip}:{host.port}</span>
                      </div>
                    </div>
                  </div>

                  <button
                    type="button"
                    disabled={disabled || isConnecting}
                    onClick={() => onConnect(ip, host.port, host.host_name)}
                    className="ml-3 px-3 py-1.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 disabled:bg-slate-800 disabled:opacity-50 text-white text-xs font-medium flex items-center gap-1.5 transition shrink-0 cursor-pointer shadow-sm shadow-emerald-950"
                  >
                    <Zap className="w-3.5 h-3.5 fill-current" />
                    <span>Connect</span>
                  </button>
                </div>
              );
            })}
          </div>
        ) : (
          <div className="flex flex-col items-center justify-center py-6 px-4 rounded-lg bg-slate-950/40 border border-dashed border-slate-800/80 text-center">
            {isDiscovering ? (
              <>
                <Search className="w-6 h-6 text-emerald-400 animate-pulse mb-2" />
                <p className="text-xs text-slate-300 font-medium">
                  Listening for MicStream Hosts on LAN...
                </p>
                <p className="text-[11px] text-slate-500 mt-1 max-w-xs">
                  Zero-config mDNS discovery is active. Ensure your host machine is running in Host mode on the same network.
                </p>
              </>
            ) : (
              <>
                <Server className="w-6 h-6 text-slate-500 mb-2" />
                <p className="text-xs text-slate-300 font-medium">No LAN Hosts Detected</p>
                <p className="text-[11px] text-slate-500 mt-1 max-w-xs">
                  Click refresh above to scan again, or enter the host IP address manually below.
                </p>
              </>
            )}
          </div>
        )}
      </div>

      {/* 2. Manual IP & Port Override Form */}
      <div className="p-4 rounded-xl bg-slate-900/70 border border-slate-800 flex flex-col gap-3 shadow-sm">
        <button
          type="button"
          onClick={() => setShowManualForm(!showManualForm)}
          className="flex items-center justify-between w-full text-left cursor-pointer"
        >
          <div className="flex items-center gap-2">
            <span className="text-xs font-semibold text-slate-200">
              Manual Target Destination
            </span>
            <span className="text-[10px] text-slate-400">
              (Direct IP fallback)
            </span>
          </div>
          <div className="text-slate-400 hover:text-slate-200 p-1">
            {showManualForm ? (
              <ChevronUp className="w-4 h-4" />
            ) : (
              <ChevronDown className="w-4 h-4" />
            )}
          </div>
        </button>

        {showManualForm && (
          <div className="flex flex-col gap-3 pt-1">
            <div className="grid grid-cols-3 gap-3">
              <div className="col-span-2 flex flex-col gap-1">
                <label
                  htmlFor="manual-ip-input"
                  className="text-[11px] font-medium text-slate-400"
                >
                  Target Host IP
                </label>
                <input
                  id="manual-ip-input"
                  type="text"
                  value={manualIp}
                  onChange={(e) => onManualIpChange(e.target.value)}
                  disabled={disabled}
                  placeholder="192.168.1.100"
                  className="px-3 py-2 rounded-lg bg-slate-950/80 border border-slate-800 text-xs text-slate-200 font-mono focus:outline-none focus:ring-1 focus:ring-emerald-500 disabled:opacity-50 transition"
                />
              </div>

              <div className="col-span-1 flex flex-col gap-1">
                <label
                  htmlFor="manual-port-input"
                  className="text-[11px] font-medium text-slate-400"
                >
                  UDP Port
                </label>
                <input
                  id="manual-port-input"
                  type="number"
                  value={manualPort}
                  onChange={(e) => onManualPortChange(Number(e.target.value))}
                  disabled={disabled}
                  min={1024}
                  max={65535}
                  placeholder="48124"
                  className="px-3 py-2 rounded-lg bg-slate-950/80 border border-slate-800 text-xs text-slate-200 font-mono focus:outline-none focus:ring-1 focus:ring-emerald-500 disabled:opacity-50 transition"
                />
              </div>
            </div>

            <div className="flex items-center justify-between gap-3 pt-1">
              <span className="text-[10px] text-slate-500">
                Default port: 48124. Works across isolated subnets and VPNs.
              </span>

              <button
                type="button"
                disabled={disabled || !isManualFormValid || isConnecting}
                onClick={() => onConnect(manualIp, manualPort)}
                className="px-4 py-2 rounded-lg bg-emerald-600 hover:bg-emerald-500 disabled:bg-slate-800 disabled:opacity-50 text-white text-xs font-medium flex items-center gap-1.5 transition cursor-pointer shadow-sm shadow-emerald-950"
              >
                <Play className="w-3.5 h-3.5 fill-current" />
                <span>Connect to IP</span>
              </button>
            </div>
          </div>
        )}

        {/* 3. Recent Connections History Chips */}
        {recentHosts.length > 0 && (
          <div className="flex flex-col gap-1.5 pt-2 border-t border-slate-800/80">
            <div className="flex items-center gap-1.5 text-[11px] text-slate-400">
              <Clock className="w-3 h-3 text-slate-400" />
              <span>Recent Connections</span>
            </div>

            <div className="flex flex-wrap gap-1.5">
              {recentHosts.map((recent) => (
                <div
                  key={`${recent.ip}:${recent.port}`}
                  className="inline-flex items-center gap-1 px-2.5 py-1 rounded-md bg-slate-950/90 border border-slate-800 text-[11px] font-mono text-slate-300 hover:border-slate-700 transition"
                >
                  <button
                    type="button"
                    disabled={disabled}
                    onClick={() => {
                      onManualIpChange(recent.ip);
                      onManualPortChange(recent.port);
                    }}
                    className="hover:text-emerald-400 transition cursor-pointer text-left"
                    title={`Use ${recent.ip}:${recent.port}`}
                  >
                    {recent.host_name ? (
                      <span className="font-sans font-medium text-slate-200 mr-1">
                        {recent.host_name}:
                      </span>
                    ) : null}
                    <span>{recent.ip}:{recent.port}</span>
                  </button>

                  <button
                    type="button"
                    disabled={disabled}
                    onClick={(e) => {
                      e.stopPropagation();
                      onRemoveRecentHost(recent.ip, recent.port);
                    }}
                    className="text-slate-500 hover:text-rose-400 ml-1 p-0.5 rounded transition cursor-pointer"
                    title="Remove from history"
                  >
                    <X className="w-2.5 h-2.5" />
                  </button>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
