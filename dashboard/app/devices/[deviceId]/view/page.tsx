"use client";

import Link from "next/link";
import { use, useEffect, useRef, useState } from "react";
import { api, WS_URL } from "@/lib/api";

type ViewState = "requesting" | "awaiting" | "starting" | "negotiating" | "waiting_frame" | "streaming" | "rejected" | "ended" | "failed";
interface Device { deviceId: string; deviceName: string; operatingSystem: string }
interface IceServer { urls: string | string[]; username?: string; credential?: string }

export default function ViewerPage({ params }: { params: Promise<{ deviceId: string }> }) {
  const { deviceId } = use(params);
  const videoRef = useRef<HTMLVideoElement>(null);
  const peerRef = useRef<RTCPeerConnection | null>(null);
  const socketRef = useRef<WebSocket | null>(null);
  const peerConnectedAtRef = useRef(0);
  const [device, setDevice] = useState<Device | null>(null);
  const [state, setState] = useState<ViewState>("requesting");
  const [stats, setStats] = useState({ resolution: "—", fps: "—", bitrate: "—", networkRtt: "—", packetLoss: "—", route: "—", protocol: "—" });
  const [error, setError] = useState("");

  useEffect(() => {
    let active = true;
    let statsTimer: number | undefined;
    let previousBytes = 0;
    let previousTimestamp = 0;
    const requestStartedAt = performance.now();
    const start = async () => {
      try {
        const [deviceResult, created] = await Promise.all([
          api<Device>(`/api/devices/${deviceId}`),
          api<{ session: { sessionId: string; trusted: boolean }; iceServers: IceServer[] }>(`/api/devices/${deviceId}/sessions`, { method: "POST" }),
        ]);
        if (!active) return;
        setDevice(deviceResult); setState(created.session.trusted ? "starting" : "awaiting");
        console.debug(`SESSION_REQUEST_MS=${(performance.now() - requestStartedAt).toFixed(1)}`);
        const sessionId = created.session.sessionId;
        const peer = new RTCPeerConnection({ iceServers: created.iceServers });
        peerRef.current = peer;
        const socket = new WebSocket(`${WS_URL}/viewer?sessionId=${encodeURIComponent(sessionId)}`);
        socketRef.current = socket;
        peer.ontrack = ({ streams }) => { if (videoRef.current && streams[0]) videoRef.current.srcObject = streams[0]; };
        peer.onicecandidate = ({ candidate }) => {
          if (candidate && socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify({ type: "ice_candidate", sessionId, candidate: candidate.candidate, sdpMid: candidate.sdpMid, sdpMLineIndex: candidate.sdpMLineIndex }));
        };
        peer.onconnectionstatechange = () => {
          if (peer.connectionState === "connected") {
            setState("waiting_frame");
            peerConnectedAtRef.current = performance.now();
            console.debug(`WEBRTC_NEGOTIATION_MS=${(performance.now() - requestStartedAt).toFixed(1)}`);
            if (socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify({ type: "session_connected", sessionId }));
          }
          if (["failed", "disconnected", "closed"].includes(peer.connectionState)) setState(peer.connectionState === "failed" ? "failed" : "ended");
        };
        socket.onmessage = async ({ data }) => {
          const message = JSON.parse(String(data)) as Record<string, unknown>;
          if (message.type === "session_state") {
            const session = message.session as { status?: string };
            if (session.status === "accepted") setState("starting");
            if (session.status === "connecting") setState("negotiating");
            if (session.status === "rejected") { setState("rejected"); setError("The remote user denied the request."); }
            if (["ended", "expired", "failed"].includes(session.status ?? "")) { setState("ended"); setError(`Session ${session.status}.`); }
          }
          if (message.type === "session_accepted") setState("starting");
          if (message.type === "session_rejected") { setState("rejected"); setError(String(message.reason ?? "The remote user denied the request.")); }
          if (message.type === "session_ended") { setState("ended"); setError(String(message.reason ?? "Session ended")); peer.close(); }
          if (message.type === "webrtc_offer") {
            setState("negotiating");
            await peer.setRemoteDescription({ type: "offer", sdp: String(message.sdp) });
            const answer = await peer.createAnswer(); await peer.setLocalDescription(answer);
            socket.send(JSON.stringify({ type: "webrtc_answer", sessionId, sdp: answer.sdp }));
          }
          if (message.type === "ice_candidate") await peer.addIceCandidate({ candidate: String(message.candidate), sdpMid: message.sdpMid as string | null, sdpMLineIndex: message.sdpMLineIndex as number | null });
        };
        statsTimer = window.setInterval(async () => {
          const reports = await peer.getStats();
          let videoStats: Partial<typeof stats> = {};
          let selectedPair: RTCStats | undefined;
          reports.forEach((report) => {
            if (report.type === "inbound-rtp" && report.kind === "video") {
              const seconds = previousTimestamp ? (report.timestamp - previousTimestamp) / 1000 : 0;
              const bitrate = seconds > 0 ? ((report.bytesReceived - previousBytes) * 8 / seconds / 1000).toFixed(0) : "—";
              previousBytes = report.bytesReceived; previousTimestamp = report.timestamp;
              videoStats = { resolution: `${report.frameWidth ?? 0}×${report.frameHeight ?? 0}`, fps: String(report.framesPerSecond ?? "—"), bitrate: bitrate === "—" ? "—" : `${bitrate} kbps`, packetLoss: String(report.packetsLost ?? 0) };
            }
            if (report.type === "candidate-pair" && report.state === "succeeded" && report.nominated) selectedPair = report;
          });
          if (selectedPair) {
            const pair = selectedPair as RTCStats & { localCandidateId?: string; remoteCandidateId?: string; currentRoundTripTime?: number };
            const local = pair.localCandidateId ? reports.get(pair.localCandidateId) : undefined;
            const remote = pair.remoteCandidateId ? reports.get(pair.remoteCandidateId) : undefined;
            videoStats.networkRtt = pair.currentRoundTripTime != null ? `${Math.round(pair.currentRoundTripTime * 1000)} ms` : "—";
            videoStats.route = local?.candidateType === "relay" || remote?.candidateType === "relay" ? "Relay" : "Direct";
            videoStats.protocol = String(local?.protocol ?? remote?.protocol ?? "—").toUpperCase();
          }
          setStats((current) => ({ ...current, ...videoStats }));
        }, 2000);
      } catch (reason) { setState("failed"); setError(reason instanceof Error ? reason.message : "Unable to start session"); }
    };
    void start();
    return () => { active = false; if (statsTimer) window.clearInterval(statsTimer); const socket = socketRef.current; if (socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify({ type: "session_end", sessionId: new URL(socket.url).searchParams.get("sessionId"), reason: "viewer_left" })); socket?.close(); peerRef.current?.close(); };
  }, [deviceId]);

  async function fullscreen() { await videoRef.current?.requestFullscreen(); }
  function videoPlaying() {
    if (state !== "streaming") {
      console.debug(`TIME_TO_FIRST_BROWSER_FRAME_MS=${(performance.now() - peerConnectedAtRef.current).toFixed(1)}`);
      setState("streaming");
    }
  }
  function disconnect() { const socket = socketRef.current; const sessionId = socket ? new URL(socket.url).searchParams.get("sessionId") : null; if (socket?.readyState === WebSocket.OPEN && sessionId) socket.send(JSON.stringify({ type: "session_end", sessionId, reason: "viewer_disconnected" })); peerRef.current?.close(); socket?.close(); setState("ended"); }

  const stateLabel = state === "waiting_frame" ? "Waiting for first frame" : state === "streaming" ? "Streaming" : state;
  return <main className="viewer"><header><Link href="/devices" className="back">← Devices</Link><div><strong>{device?.deviceName ?? "Remote device"}</strong><span className={`connection ${state}`}>{stateLabel}</span></div><button className="secondary" onClick={disconnect}>Disconnect</button></header>
    <section className="screen"><video ref={videoRef} autoPlay playsInline onPlaying={videoPlaying} />{state !== "streaming" && <div className="screen-message"><strong>{state === "awaiting" ? "Waiting for authorization" : state === "starting" ? "Starting capture" : state === "negotiating" ? "Negotiating WebRTC" : state === "waiting_frame" ? "WebRTC connected — waiting for first video frame" : state === "requesting" ? "Requesting session" : ""}</strong>{error && <p>{error}</p>}</div>}</section>
    <footer><div><span>Resolution<strong>{stats.resolution}</strong></span><span>FPS<strong>{stats.fps}</strong></span><span>Bitrate<strong>{stats.bitrate}</strong></span><span>Network RTT<strong>{stats.networkRtt}</strong></span><span>Packet loss<strong>{stats.packetLoss}</strong></span><span>ICE route<strong>{stats.route}</strong></span><span>Protocol<strong>{stats.protocol}</strong></span></div><button className="secondary" onClick={() => void fullscreen()}>Fullscreen</button></footer>
  </main>;
}
