import { useEffect, useState, useRef } from "react";
import { Typography, Card, Tag, Empty, Spin } from "antd";
import { LoadingOutlined } from "@ant-design/icons";
import { getToken } from "../store/auth";

const { Text, Title } = Typography;

interface LogEntry {
  timestamp: string;
  level: string;
  message: string;
  target: string;
}

function levelColor(level: string): string {
  switch (level) {
    case "ERROR":
    case "error":
      return "red";
    case "WARN":
    case "warn":
      return "orange";
    case "INFO":
    case "info":
      return "blue";
    case "DEBUG":
    case "debug":
      return "default";
    default:
      return "default";
  }
}

function formatTime(ts: string): string {
  try {
    const d = new Date(ts);
    return d.toLocaleTimeString("es-ES", { hour12: false });
  } catch {
    return ts;
  }
}

export default function LogsPage() {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const MAX_LOGS = 500;

  useEffect(() => {
    const token = getToken();
    if (!token) {
      setError("No authentication token available");
      return;
    }

    const eventSource = new EventSource("/api/logs/stream");

    eventSource.onopen = () => {
      setConnected(true);
      setError(null);
    };

    eventSource.onmessage = (event) => {
      try {
        const entry: LogEntry = JSON.parse(event.data);
        setLogs((prev) => {
          const next = [entry, ...prev];
          return next.length > MAX_LOGS ? next.slice(0, MAX_LOGS) : next;
        });
      } catch {
        // Ignore parse errors (e.g., ping keepalive messages)
      }
    };

    eventSource.onerror = () => {
      setConnected(false);
      setError("Connection lost. The server may be unavailable.");
    };

    return () => {
      eventSource.close();
    };
  }, []);

  // Auto-scroll to bottom on new logs
  useEffect(() => {
    if (containerRef.current) {
      containerRef.current.scrollTop = 0; // New logs are prepended
    }
  }, [logs]);

  return (
    <div>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: 16,
        }}
      >
        <Title level={3} style={{ margin: 0 }}>
          Logs
        </Title>
        <div>
          {connected ? (
            <Tag color="green" icon={<LoadingOutlined />}>
              Connected
            </Tag>
          ) : (
            <Tag color="red">Disconnected</Tag>
          )}
        </div>
      </div>

      {error && (
        <Card size="small" style={{ marginBottom: 16, borderColor: "#ff4d4f" }}>
          <Text type="danger">{error}</Text>
        </Card>
      )}

      {logs.length === 0 && !error ? (
        <div style={{ textAlign: "center", padding: 60 }}>
          <Spin size="large" />
          <br />
          <Text type="secondary" style={{ marginTop: 16, display: "block" }}>
            Waiting for log entries...
          </Text>
          <Text type="secondary" style={{ display: "block", fontSize: 12 }}>
            Try running a feed manually to see logs appear here.
          </Text>
        </div>
      ) : (
        <div
          ref={containerRef}
          style={{
            maxHeight: "calc(100vh - 220px)",
            overflowY: "auto",
            background: "#1a1a2e",
            borderRadius: 8,
            padding: 8,
          }}
        >
          {logs.map((entry, i) => (
            <div
              key={`${entry.timestamp}-${i}`}
              style={{
                fontFamily: "monospace",
                fontSize: 13,
                padding: "2px 6px",
                color: "#e0e0e0",
                lineHeight: 1.6,
                whiteSpace: "pre-wrap",
                wordBreak: "break-all",
              }}
            >
              <Text
                style={{ color: "#888", fontFamily: "monospace", fontSize: 12 }}
              >
                {formatTime(entry.timestamp)}
              </Text>
              <Tag color={levelColor(entry.level)} style={{ fontSize: 11 }}>
                {entry.level}
              </Tag>
              <Text style={{ color: "#aaa", fontSize: 11 }}>[{entry.target}]</Text>{" "}
              <Text style={{ color: "#e0e0e0" }}>{entry.message}</Text>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}