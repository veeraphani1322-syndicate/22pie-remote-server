"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api } from "@/lib/api";

interface Device { deviceId: string; deviceName: string; operatingSystem: string; agentVersion: string; status: "online" | "offline"; lastSeenAt: string }

export default function DevicesPage() {
  const router = useRouter();
  const [devices, setDevices] = useState<Device[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let active = true;
    const load = () => api<{ devices: Device[] }>("/api/devices")
      .then((result) => { if (active) setDevices(result.devices); })
      .catch(() => { if (active) router.replace("/login"); })
      .finally(() => { if (active) setLoading(false); });
    void load();
    const timer = window.setInterval(load, 5000);
    return () => { active = false; window.clearInterval(timer); };
  }, [router]);

  return <main className="shell"><header><div className="brand">22Pie <span>Remote</span></div><div className="secure">● Secure console</div></header>
    <section className="page-heading"><p className="eyebrow">Workspace</p><h1>My devices</h1><p>Live screen viewing requires approval on the remote computer.</p></section>
    {loading ? <div className="empty">Loading devices…</div> : devices.length === 0 ? <div className="empty">No registered devices yet.</div> :
      <div className="device-grid">{devices.map((device) => <article className="device-card" key={device.deviceId}>
        <div className="device-icon">▣</div><div><h2>{device.deviceName}</h2><p>{device.operatingSystem}</p><p>Agent {device.agentVersion}</p></div>
        <div className={`status ${device.status}`}><i />{device.status === "online" ? "Online" : "Offline"}</div>
        {device.status === "online" ? <Link className="action" href={`/devices/${device.deviceId}/view`}>View screen</Link> : <span className="action disabled">View screen</span>}
      </article>)}</div>}
  </main>;
}
