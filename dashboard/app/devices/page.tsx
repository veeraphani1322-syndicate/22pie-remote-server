"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api";

interface Device {
  deviceId: string; deviceName: string; operatingSystem: string; agentVersion: string;
  status: "online" | "offline"; lastSeenAt: string;
  trustedScreenAccess: boolean;
  trustedAccess?: { permission: "SCREEN_VIEW"; createdAt: string; userEmail: string };
}
type PageState = "loading" | "ready" | "unauthorized" | "unavailable";

export default function DevicesPage() {
  const router = useRouter();
  const [devices, setDevices] = useState<Device[]>([]);
  const [state, setState] = useState<PageState>("loading");

  const load = useCallback(async () => {
    setState("loading");
    try {
      await api("/api/auth/me");
      const result = await api<{ devices: Device[] }>("/api/devices");
      setDevices(result.devices);
      setState("ready");
    } catch (reason) {
      if (reason instanceof ApiError && reason.status === 401) {
        setState("unauthorized");
        router.replace("/login");
      } else setState("unavailable");
    }
  }, [router]);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => {
    if (state !== "ready") return;
    const timer = window.setInterval(() => {
      api<{ devices: Device[] }>("/api/devices")
        .then((result) => setDevices(result.devices))
        .catch((reason) => {
          if (reason instanceof ApiError && reason.status === 401) {
            setState("unauthorized");
            router.replace("/login");
          } else setState("unavailable");
        });
    }, 5000);
    return () => window.clearInterval(timer);
  }, [router, state]);

  async function logout() {
    try { await api("/api/auth/logout", { method: "POST" }); }
    finally { router.replace("/login"); }
  }

  async function revokeTrust(deviceId: string) {
    if (!window.confirm("Revoke trusted screen access? Future sessions will require approval on the remote computer.")) return;
    await api(`/api/devices/${deviceId}/trusted-access/SCREEN_VIEW`, { method: "DELETE" });
    setDevices((current) => current.map((device) => device.deviceId === deviceId ? { ...device, trustedScreenAccess: false } : device));
  }

  return <main className="shell"><header><div className="brand">22Pie <span>Remote</span></div><div className="header-actions"><div className="secure">● Secure console</div><button className="text-button" onClick={() => void logout()}>Sign out</button></div></header>
    <section className="page-heading"><p className="eyebrow">Workspace</p><h1>My devices</h1><p>Screen viewing requires approval once unless this account has trusted access.</p></section>
    {state === "loading" && <div className="empty" role="status">Loading…</div>}
    {state === "unauthorized" && <div className="empty" role="alert"><strong>Session expired</strong><p>Redirecting to login…</p></div>}
    {state === "unavailable" && <div className="empty" role="alert"><strong>Unable to connect to server</strong><p>Check that the API is running and this dashboard origin is allowed.</p><button onClick={() => void load()}>Retry</button></div>}
    {state === "ready" && devices.length === 0 && <div className="empty">No registered devices yet.</div>}
    {state === "ready" && devices.length > 0 && <div className="device-grid">{devices.map((device) => <article className="device-card" key={device.deviceId}>
      <div className="device-icon">▣</div><div><h2>{device.deviceName}</h2><p>{device.operatingSystem}</p><p>Agent {device.agentVersion}</p></div>
      <div className={`status ${device.status}`}><i />{device.status === "online" ? "Online" : "Offline"}</div>
      {device.trustedScreenAccess && <div><p>Trusted Screen Access ✓</p><p>Trusted for: {device.trustedAccess?.userEmail}</p><p>Granted: {device.trustedAccess ? new Date(device.trustedAccess.createdAt).toLocaleString() : "—"}</p><button className="text-button" onClick={() => void revokeTrust(device.deviceId)}>Revoke</button></div>}
      {device.status === "online" ? <Link className="action" href={`/devices/${device.deviceId}/view`}>View screen</Link> : <span className="action disabled">View screen</span>}
    </article>)}</div>}
  </main>;
}
