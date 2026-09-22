import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { DiscoveredHost, RecentHost } from "../types";

export const STORAGE_KEY_RECENT_HOSTS = "micstream:recent-hosts";
const MAX_RECENT_HOSTS = 8;

export interface UseDiscoveryResult {
  isDiscovering: boolean;
  discoveredHosts: DiscoveredHost[];
  recentHosts: RecentHost[];
  error: string | null;
  startDiscovery: () => Promise<void>;
  stopDiscovery: () => Promise<void>;
  addRecentHost: (ip: string, port: number, host_name?: string) => void;
  removeRecentHost: (ip: string, port: number) => void;
  clearRecentHosts: () => void;
  clearDiscoveredHosts: () => void;
}

function loadRecentHostsFromStorage(): RecentHost[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY_RECENT_HOSTS);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (Array.isArray(parsed)) {
      return parsed.filter(
        (item): item is RecentHost =>
          item &&
          typeof item.ip === "string" &&
          item.ip.trim().length > 0 &&
          typeof item.port === "number" &&
          item.port > 0 &&
          item.port <= 65535
      );
    }
  } catch (err) {
    console.warn("Failed to load recent hosts from localStorage:", err);
  }
  return [];
}

function saveRecentHostsToStorage(hosts: RecentHost[]) {
  try {
    localStorage.setItem(STORAGE_KEY_RECENT_HOSTS, JSON.stringify(hosts));
  } catch (err) {
    console.warn("Failed to save recent hosts to localStorage:", err);
  }
}

export function useDiscovery(): UseDiscoveryResult {
  const [isDiscovering, setIsDiscovering] = useState<boolean>(false);
  const [discoveredHosts, setDiscoveredHosts] = useState<DiscoveredHost[]>([]);
  const [recentHosts, setRecentHosts] = useState<RecentHost[]>(loadRecentHostsFromStorage);
  const [error, setError] = useState<string | null>(null);

  const isMountedRef = useRef<boolean>(true);
  const unlistenRef = useRef<UnlistenFn | null>(null);

  const clearDiscoveredHosts = useCallback(() => {
    setDiscoveredHosts([]);
  }, []);

  const stopDiscovery = useCallback(async () => {
    if (unlistenRef.current) {
      unlistenRef.current();
      unlistenRef.current = null;
    }

    try {
      await invoke("stop_discovery");
    } catch (err) {
      console.warn("Failed to stop mDNS discovery:", err);
    }

    if (isMountedRef.current) {
      setIsDiscovering(false);
    }
  }, []);

  const startDiscovery = useCallback(async () => {
    setError(null);

    // If already listening, stop previous subscription first
    if (unlistenRef.current) {
      unlistenRef.current();
      unlistenRef.current = null;
    }

    let isSubscribed = true;

    try {
      const unlisten = await listen<DiscoveredHost>("host-discovered", (event) => {
        if (!isSubscribed || !isMountedRef.current) return;
        const incoming = event.payload;
        if (!incoming || !incoming.host_name) return;

        setDiscoveredHosts((prev) => {
          const index = prev.findIndex(
            (h) =>
              h.host_name.toLowerCase() === incoming.host_name.toLowerCase() ||
              (h.ip_addresses[0] &&
                incoming.ip_addresses[0] &&
                h.ip_addresses[0] === incoming.ip_addresses[0] &&
                h.port === incoming.port)
          );

          if (index >= 0) {
            const updated = [...prev];
            const mergedIps = Array.from(
              new Set([...updated[index].ip_addresses, ...incoming.ip_addresses])
            );
            updated[index] = {
              ...incoming,
              ip_addresses: mergedIps,
            };
            return updated;
          }

          return [...prev, incoming];
        });
      });

      if (!isMountedRef.current) {
        unlisten();
        return;
      }

      unlistenRef.current = unlisten;

      await invoke("start_discovery");
      if (isMountedRef.current) {
        setIsDiscovering(true);
      }
    } catch (err) {
      isSubscribed = false;
      if (unlistenRef.current) {
        unlistenRef.current();
        unlistenRef.current = null;
      }
      const message = err instanceof Error ? err.message : String(err);
      if (isMountedRef.current) {
        setError(message);
        setIsDiscovering(false);
      }
    }
  }, []);

  const addRecentHost = useCallback(
    (ip: string, port: number, host_name?: string) => {
      const trimmedIp = ip.trim();
      if (!trimmedIp || port <= 0 || port > 65535) return;

      setRecentHosts((prev) => {
        const filtered = prev.filter(
          (h) => !(h.ip.toLowerCase() === trimmedIp.toLowerCase() && h.port === port)
        );
        const updated = [
          {
            host_name: host_name?.trim() || undefined,
            ip: trimmedIp,
            port,
            last_connected: Date.now(),
          },
          ...filtered,
        ].slice(0, MAX_RECENT_HOSTS);

        saveRecentHostsToStorage(updated);
        return updated;
      });
    },
    []
  );

  const removeRecentHost = useCallback((ip: string, port: number) => {
    setRecentHosts((prev) => {
      const updated = prev.filter(
        (h) => !(h.ip.toLowerCase() === ip.toLowerCase() && h.port === port)
      );
      saveRecentHostsToStorage(updated);
      return updated;
    });
  }, []);

  const clearRecentHosts = useCallback(() => {
    setRecentHosts([]);
    try {
      localStorage.removeItem(STORAGE_KEY_RECENT_HOSTS);
    } catch (err) {
      console.warn("Failed to clear recent hosts:", err);
    }
  }, []);

  // Clean teardown on unmount
  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
      if (unlistenRef.current) {
        unlistenRef.current();
        unlistenRef.current = null;
      }
      invoke("stop_discovery").catch(() => {
        // Ignore background unmount stop failures
      });
    };
  }, []);

  return {
    isDiscovering,
    discoveredHosts,
    recentHosts,
    error,
    startDiscovery,
    stopDiscovery,
    addRecentHost,
    removeRecentHost,
    clearRecentHosts,
    clearDiscoveredHosts,
  };
}
