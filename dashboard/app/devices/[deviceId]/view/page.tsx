"use client";

import Link from "next/link";
import { use, useEffect, useRef, useState } from "react";
import { api, WS_URL } from "@/lib/api";
import { normalizedVideoPoint, type Point } from "@/lib/mouse-control";
import { isLocalKeyboardRelease, modifiersOf, shouldSendText } from "@/lib/keyboard-control";

type ViewState = "requesting" | "awaiting" | "starting" | "negotiating" | "waiting_frame" | "streaming" | "rejected" | "ended" | "failed";
interface Device { deviceId: string; deviceName: string; operatingSystem: string }
interface IceServer { urls: string | string[]; username?: string; credential?: string }

export default function ViewerPage({ params }: { params: Promise<{ deviceId: string }> }) {
  const { deviceId } = use(params);
  const videoRef = useRef<HTMLVideoElement>(null);
  const peerRef = useRef<RTCPeerConnection | null>(null);
  const socketRef = useRef<WebSocket | null>(null);
  const channelRef = useRef<RTCDataChannel | null>(null);
  const sessionIdRef = useRef<string | null>(null);
  const moveRef = useRef<Point | null>(null);
  const moveFrameRef = useRef<number | null>(null);
  const sequenceRef = useRef(0);
  const heldKeysRef = useRef(new Set<string>());
  const peerConnectedAtRef = useRef(0);
  const [device, setDevice] = useState<Device | null>(null);
  const [state, setState] = useState<ViewState>("requesting");
  const [stats, setStats] = useState({ resolution: "—", fps: "—", bitrate: "—", networkRtt: "—", packetLoss: "—", route: "—", protocol: "—" });
  const [error, setError] = useState("");
  const [mouseAuthorized, setMouseAuthorized] = useState(false);
  const [mouseEnabled, setMouseEnabled] = useState(false);
  const [mouseRequesting, setMouseRequesting] = useState(false);
  const [keyboardAuthorized, setKeyboardAuthorized] = useState(false);
  const [keyboardEnabled, setKeyboardEnabled] = useState(false);
  const [keyboardRequesting, setKeyboardRequesting] = useState(false);
  const [keyboardCaptured, setKeyboardCaptured] = useState(false);
  const [keyboardWarning, setKeyboardWarning] = useState("");
  const [controlChannel, setControlChannel] = useState<"connecting" | "connected" | "disconnected">("connecting");

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
        sessionIdRef.current = sessionId;
        const peer = new RTCPeerConnection({ iceServers: created.iceServers });
        peerRef.current = peer;
        const socket = new WebSocket(`${WS_URL}/viewer?sessionId=${encodeURIComponent(sessionId)}`);
        socketRef.current = socket;
        const pendingLocalCandidates: RTCIceCandidate[] = [];
        let signalingChain = Promise.resolve();
        const sendSignal = (message: Record<string, unknown>, diagnostic: string) => {
          if (socket.readyState !== WebSocket.OPEN) throw new Error(`Viewer WebSocket is not open for ${diagnostic}`);
          socket.send(JSON.stringify(message));
          console.debug(`${diagnostic} sessionId=${sessionId}`);
        };
        socket.onopen = () => {
          console.debug(`VIEWER_WS_OPEN sessionId=${sessionId}`);
          for (const candidate of pendingLocalCandidates.splice(0)) sendSignal({ type: "ice_candidate", sessionId, candidate: candidate.candidate, sdpMid: candidate.sdpMid, sdpMLineIndex: candidate.sdpMLineIndex }, "ICE_CANDIDATE_SENT");
        };
        socket.onclose = ({ code }) => {
          console.debug(`VIEWER_WS_CLOSE sessionId=${sessionId} code=${code}`);
          if (active && state !== "ended") setState("ended");
        };
        socket.onerror = () => {
          console.error(`VIEWER_WS_ERROR sessionId=${sessionId}`);
          if (active) { setState("failed"); setError("Viewer signaling connection failed."); }
        };
        peer.ontrack = ({ streams }) => { if (videoRef.current && streams[0]) videoRef.current.srcObject = streams[0]; };
        peer.ondatachannel = ({ channel }) => {
          if (channel.label !== "control") { channel.close(); return; }
          channelRef.current = channel;
          channel.onopen = () => { console.debug(`CONTROL_CHANNEL_STATE=OPEN sessionId=${sessionId}`); setControlChannel("connected"); };
          channel.onclose = () => { console.debug(`CONTROL_CHANNEL_STATE=CLOSED sessionId=${sessionId}`); heldKeysRef.current.clear(); setControlChannel("disconnected"); setMouseEnabled(false); setKeyboardEnabled(false); setKeyboardCaptured(false); };
          channel.onerror = () => { console.error(`CONTROL_CHANNEL_STATE=ERROR sessionId=${sessionId}`); heldKeysRef.current.clear(); setControlChannel("disconnected"); setMouseEnabled(false); setKeyboardEnabled(false); setKeyboardCaptured(false); };
        };
        peer.onicecandidate = ({ candidate }) => {
          if (!candidate) return;
          if (socket.readyState === WebSocket.OPEN) sendSignal({ type: "ice_candidate", sessionId, candidate: candidate.candidate, sdpMid: candidate.sdpMid, sdpMLineIndex: candidate.sdpMLineIndex }, "ICE_CANDIDATE_SENT");
          else { if (pendingLocalCandidates.length === 64) pendingLocalCandidates.shift(); pendingLocalCandidates.push(candidate); }
        };
        peer.oniceconnectionstatechange = () => console.debug(`ICE_CONNECTION_STATE=${peer.iceConnectionState} sessionId=${sessionId}`);
        peer.onconnectionstatechange = () => {
          console.debug(`PEER_CONNECTION_STATE=${peer.connectionState} sessionId=${sessionId}`);
          if (peer.connectionState === "connected") {
            setState("waiting_frame");
            peerConnectedAtRef.current = performance.now();
            console.debug(`WEBRTC_NEGOTIATION_MS=${(performance.now() - requestStartedAt).toFixed(1)}`);
            if (socket.readyState === WebSocket.OPEN) sendSignal({ type: "session_connected", sessionId }, "SESSION_CONNECTED_SENT");
          }
          if (["failed", "disconnected", "closed"].includes(peer.connectionState)) setState(peer.connectionState === "failed" ? "failed" : "ended");
        };
        socket.onmessage = ({ data }) => {
          signalingChain = signalingChain.then(async () => {
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
          if (message.type === "mouse_control_authorized") { setMouseAuthorized(true); setMouseEnabled(true); setMouseRequesting(false); }
          if (message.type === "mouse_control_rejected") { setMouseAuthorized(false); setMouseEnabled(false); setMouseRequesting(false); setError(String(message.reason ?? "Mouse control denied")); }
          if (message.type === "mouse_control_disabled") { setMouseEnabled(false); }
          if (message.type === "keyboard_control_authorized") { setKeyboardAuthorized(true); setKeyboardEnabled(true); setKeyboardRequesting(false); }
          if (message.type === "keyboard_control_rejected") { setKeyboardAuthorized(false); setKeyboardEnabled(false); setKeyboardRequesting(false); setKeyboardCaptured(false); setError(String(message.reason ?? "Keyboard control denied")); }
          if (message.type === "keyboard_control_disabled") { setKeyboardEnabled(false); setKeyboardCaptured(false); heldKeysRef.current.clear(); }
          if (message.type === "webrtc_offer") {
            console.debug(`WEBRTC_OFFER_RECEIVED sessionId=${sessionId}`);
            setState("negotiating");
            await peer.setRemoteDescription({ type: "offer", sdp: String(message.sdp) });
            console.debug(`WEBRTC_REMOTE_DESCRIPTION_SET sessionId=${sessionId}`);
            const answer = await peer.createAnswer();
            console.debug(`WEBRTC_ANSWER_CREATED sessionId=${sessionId}`);
            await peer.setLocalDescription(answer);
            sendSignal({ type: "webrtc_answer", sessionId, sdp: answer.sdp }, "WEBRTC_ANSWER_SENT");
          }
          if (message.type === "ice_candidate") {
            console.debug(`ICE_CANDIDATE_RECEIVED sessionId=${sessionId}`);
            await peer.addIceCandidate({ candidate: String(message.candidate), sdpMid: message.sdpMid as string | null, sdpMLineIndex: message.sdpMLineIndex as number | null });
          }
          }).catch((reason) => {
            console.error(`VIEWER_SIGNALING_ERROR sessionId=${sessionId}`, reason);
            if (active) { setState("failed"); setError(reason instanceof Error ? reason.message : "WebRTC signaling failed"); }
          });
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
    return () => { active = false; if (statsTimer) window.clearInterval(statsTimer); if (moveFrameRef.current != null) cancelAnimationFrame(moveFrameRef.current); channelRef.current?.close(); const socket = socketRef.current; if (socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify({ type: "session_end", sessionId: new URL(socket.url).searchParams.get("sessionId"), reason: "viewer_left" })); socket?.close(); peerRef.current?.close(); };
  }, [deviceId]);

  async function fullscreen() { await videoRef.current?.requestFullscreen(); }
  function videoPlaying() {
    if (state !== "streaming") {
      console.debug(`TIME_TO_FIRST_BROWSER_FRAME_MS=${(performance.now() - peerConnectedAtRef.current).toFixed(1)}`);
      setState("streaming");
    }
  }
  function disconnect() { const socket = socketRef.current; const sessionId = socket ? new URL(socket.url).searchParams.get("sessionId") : null; if (socket?.readyState === WebSocket.OPEN && sessionId) socket.send(JSON.stringify({ type: "session_end", sessionId, reason: "viewer_disconnected" })); peerRef.current?.close(); socket?.close(); setState("ended"); }

  function sendMouse(type: string, fields: Record<string, unknown>, move = false) {
    const channel = channelRef.current; const sessionId = sessionIdRef.current;
    if (!mouseEnabled || !mouseAuthorized || !sessionId || channel?.readyState !== "open") return;
    if (move && channel.bufferedAmount > 64 * 1024) return;
    channel.send(JSON.stringify({ type, session_id: sessionId, ...fields, sequence: ++sequenceRef.current, timestamp: performance.now() }));
  }
  function pointer(event: React.MouseEvent<HTMLVideoElement>) { const video = videoRef.current; return video ? normalizedVideoPoint(event.clientX, event.clientY, video.getBoundingClientRect(), video.videoWidth, video.videoHeight) : null; }
  function mouseMove(event: React.MouseEvent<HTMLVideoElement>) {
    const point = pointer(event); if (!point) return; moveRef.current = point;
    if (moveFrameRef.current != null) return;
    moveFrameRef.current = requestAnimationFrame(() => { moveFrameRef.current = null; const latest = moveRef.current; if (latest) sendMouse("mouse_move", { x: latest.x, y: latest.y }, true); });
  }
  function buttonName(button: number) { return button === 0 ? "left" : button === 1 ? "middle" : button === 2 ? "right" : null; }
  function mouseButton(event: React.MouseEvent<HTMLVideoElement>, down: boolean) { const button = buttonName(event.button); if (!button) return; event.preventDefault(); sendMouse(down ? "mouse_down" : "mouse_up", { button }); }
  function releaseButtons() { for (const button of ["left", "right", "middle"]) sendMouse("mouse_up", { button }); }
  function mouseWheel(event: React.WheelEvent<HTMLVideoElement>) { event.preventDefault(); sendMouse("mouse_scroll", { dx: Math.max(-1200, Math.min(1200, Math.round(-event.deltaX))), dy: Math.max(-1200, Math.min(1200, Math.round(-event.deltaY))) }); }
  async function enableMouse() {
    const sessionId = sessionIdRef.current; if (!sessionId) return; setMouseRequesting(true); setError("");
    try { const result = await api<{ session: { permissions: string[] } }>(`/api/sessions/${sessionId}/mouse-control`, { method: "POST" }); if (result.session.permissions.includes("MOUSE_CONTROL")) { setMouseAuthorized(true); setMouseEnabled(true); setMouseRequesting(false); } } catch (reason) { setMouseRequesting(false); setError(reason instanceof Error ? reason.message : "Unable to request mouse control"); }
  }
  async function disableMouse() { const sessionId = sessionIdRef.current; setMouseEnabled(false); if (sessionId) await api(`/api/sessions/${sessionId}/mouse-control`, { method: "DELETE" }); }

  function sendKeyboard(type: string, fields: Record<string, unknown>, force = false) {
    const channel=channelRef.current; const sessionId=sessionIdRef.current;
    if (!keyboardEnabled || !keyboardAuthorized || !sessionId || channel?.readyState!=="open") return false;
    if (!force && channel.bufferedAmount > 256*1024) { setKeyboardWarning("Keyboard channel is congested. Release focus and retry."); return false; }
    channel.send(JSON.stringify({ type, session_id:sessionId, ...fields, sequence:++sequenceRef.current, timestamp:performance.now() })); return true;
  }
  function releaseKeys() { for (const code of heldKeysRef.current) sendKeyboard("key_up",{code,key:code,modifiers:{alt:false,ctrl:false,meta:false,shift:false}},true); heldKeysRef.current.clear(); }
  function keyboardDown(event: React.KeyboardEvent<HTMLVideoElement>) {
    if (!keyboardEnabled || !keyboardCaptured) return;
    if (isLocalKeyboardRelease(event.nativeEvent)) { event.preventDefault(); releaseKeys(); setKeyboardCaptured(false); videoRef.current?.blur(); return; }
    event.preventDefault(); setKeyboardWarning("");
    if (shouldSendText(event.nativeEvent)) { sendKeyboard("text_input",{text:event.key}); return; }
    if (sendKeyboard("key_down",{code:event.code,key:event.key,repeat:event.repeat,modifiers:modifiersOf(event.nativeEvent)})) heldKeysRef.current.add(event.code);
  }
  function keyboardUp(event: React.KeyboardEvent<HTMLVideoElement>) {
    if (!keyboardEnabled || !keyboardCaptured || !heldKeysRef.current.has(event.code)) return;
    event.preventDefault(); sendKeyboard("key_up",{code:event.code,key:event.key,modifiers:modifiersOf(event.nativeEvent)},true); heldKeysRef.current.delete(event.code);
  }
  async function enableKeyboard() { const sessionId=sessionIdRef.current; if (!sessionId) return; setKeyboardRequesting(true); setError(""); try { const result=await api<{session:{permissions:string[]}}>(`/api/sessions/${sessionId}/keyboard-control`,{method:"POST"}); if(result.session.permissions.includes("KEYBOARD_CONTROL")){setKeyboardAuthorized(true);setKeyboardEnabled(true);setKeyboardRequesting(false);} } catch(reason){setKeyboardRequesting(false);setError(reason instanceof Error?reason.message:"Unable to request keyboard control");} }
  async function disableKeyboard() { const sessionId=sessionIdRef.current; releaseKeys(); setKeyboardEnabled(false); setKeyboardCaptured(false); if(sessionId) await api(`/api/sessions/${sessionId}/keyboard-control`,{method:"DELETE"}); }

  const stateLabel = state === "waiting_frame" ? "Waiting for first frame" : state === "streaming" ? "Streaming" : state;
  return <main className="viewer"><header><Link href="/devices" className="back">← Devices</Link><div><strong>{device?.deviceName ?? "Remote device"}</strong><span className={`connection ${state}`}>{stateLabel}</span></div><button className="secondary" onClick={disconnect}>Disconnect</button></header>
    <section className="screen"><video ref={videoRef} tabIndex={0} autoPlay playsInline onPlaying={videoPlaying} onClick={() => { if(keyboardEnabled){videoRef.current?.focus();setKeyboardCaptured(true);} }} onBlur={() => {releaseKeys();setKeyboardCaptured(false);}} onKeyDown={keyboardDown} onKeyUp={keyboardUp} onMouseMove={mouseMove} onMouseDown={(event) => mouseButton(event, true)} onMouseUp={(event) => mouseButton(event, false)} onMouseLeave={releaseButtons} onContextMenu={(event) => mouseEnabled && event.preventDefault()} onWheel={mouseWheel} />{state !== "streaming" && <div className="screen-message"><strong>{state === "awaiting" ? "Waiting for authorization" : state === "starting" ? "Starting capture" : state === "negotiating" ? "Negotiating WebRTC" : state === "waiting_frame" ? "WebRTC connected — waiting for first video frame" : state === "requesting" ? "Requesting session" : ""}</strong>{error && <p>{error}</p>}</div>}{keyboardEnabled && <div className="keyboard-status" role="status">Keyboard control {keyboardCaptured ? "captured — Ctrl+Alt+Esc releases" : "enabled — click the screen to capture"}{keyboardWarning && <p>{keyboardWarning}</p>}</div>}</section>
    <footer><div><span>Resolution<strong>{stats.resolution}</strong></span><span>FPS<strong>{stats.fps}</strong></span><span>Bitrate<strong>{stats.bitrate}</strong></span><span>Network RTT<strong>{stats.networkRtt}</strong></span><span>Packet loss<strong>{stats.packetLoss}</strong></span><span>ICE route<strong>{stats.route}</strong></span><span>Protocol<strong>{stats.protocol}</strong></span><span>Control channel<strong>{controlChannel}</strong></span><span>Mouse Control<strong>{mouseEnabled ? "ON" : "OFF"}</strong></span><span>Keyboard Control<strong>{keyboardEnabled ? (keyboardCaptured ? "CAPTURED" : "ON") : "OFF"}</strong></span></div>{mouseEnabled ? <button className="secondary" onClick={() => void disableMouse()}>Disable Mouse Control</button> : <button className="secondary" disabled={mouseRequesting || state !== "streaming" || controlChannel !== "connected"} onClick={() => void enableMouse()}>{mouseRequesting ? "Waiting for approval…" : "Enable Mouse Control"}</button>}{keyboardEnabled ? <button className="secondary" onClick={() => void disableKeyboard()}>Disable Keyboard Control</button> : <button className="secondary" disabled={keyboardRequesting||state!=="streaming"||controlChannel!=="connected"} onClick={() => void enableKeyboard()}>{keyboardRequesting?"Waiting for approval…":"Enable Keyboard Control"}</button>}<button className="secondary" onClick={() => void fullscreen()}>Fullscreen</button></footer>
  </main>;
}
