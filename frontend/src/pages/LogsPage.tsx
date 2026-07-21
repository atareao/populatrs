import { useEffect, useState, useRef } from "react";
import {
  Typography, Card, Tag, Empty, Spin, Table, Tabs, Badge, Tooltip, InputNumber, Button, message,
} from "antd";
import {
  LoadingOutlined, CheckCircleOutlined, CloseCircleOutlined, ReloadOutlined, SaveOutlined,
} from "@ant-design/icons";
import { getToken } from "../store/auth";
import {
  fetchFeedLogs, updateLogRetention, fetchLogRetention,
  type FeedLogEntry, type FeedLogResponse,
} from "../api/http";

const { Text, Title } = Typography;

// ───── SSE Real-time Logs ─────

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

function formatDate(ts: string): string {
  try {
    const d = new Date(ts);
    return d.toLocaleString("es-ES", {
      year: "numeric", month: "2-digit", day: "2-digit",
      hour: "2-digit", minute: "2-digit", hour12: false,
    });
  } catch {
    return ts;
  }
}

function SseLogs() {
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

  // Auto-scroll
  useEffect(() => {
    if (containerRef.current) {
      containerRef.current.scrollTop = 0;
    }
  }, [logs]);

  return (
    <div>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 8 }}>
        <Text strong>Real-time Logs</Text>
        {connected ? (
          <Tag color="green" icon={<LoadingOutlined />}>Connected</Tag>
        ) : (
          <Tag color="red">Disconnected</Tag>
        )}
      </div>

      {error && (
        <Card size="small" style={{ marginBottom: 8, borderColor: "#ff4d4f" }}>
          <Text type="danger">{error}</Text>
        </Card>
      )}

      {logs.length === 0 && !error ? (
        <div style={{ textAlign: "center", padding: 40 }}>
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
            maxHeight: "calc(100vh - 320px)",
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
              <Text style={{ color: "#888", fontFamily: "monospace", fontSize: 12 }}>
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

// ───── Historical Feed Logs ─────

function FeedLogs() {
  const [data, setData] = useState<FeedLogResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [retentionDays, setRetentionDays] = useState(30);
  const [savingRetention, setSavingRetention] = useState(false);

  const loadData = async () => {
    setLoading(true);
    try {
      const result = await fetchFeedLogs(100, 0);
      setData(result);
    } catch (e) {
      message.error("Failed to load feed logs");
    } finally {
      setLoading(false);
    }
  };

  const loadRetention = async () => {
    try {
      const result = await fetchLogRetention();
      setRetentionDays(result.retention_days);
    } catch {
      // ignore
    }
  };

  useEffect(() => {
    loadData();
    loadRetention();
  }, []);

  const handleSaveRetention = async () => {
    setSavingRetention(true);
    try {
      await updateLogRetention(retentionDays);
      message.success("Retention updated");
    } catch {
      message.error("Failed to update retention");
    } finally {
      setSavingRetention(false);
    }
  };

  const columns = [
    {
      title: "Date",
      dataIndex: "published_at",
      key: "published_at",
      width: 160,
      render: (v: string) => <Text style={{ fontSize: 12 }}>{formatDate(v)}</Text>,
    },
    {
      title: "Feed",
      dataIndex: "feed_id",
      key: "feed_id",
      width: 120,
      render: (v: string) => <Tag>{v}</Tag>,
    },
    {
      title: "Title",
      dataIndex: "title",
      key: "title",
      ellipsis: true,
      render: (v: string, record: FeedLogEntry) => (
        <a href={record.url} target="_blank" rel="noopener noreferrer" style={{ fontSize: 13 }}>
          {v}
        </a>
      ),
    },
    {
      title: "Results",
      key: "results",
      width: 200,
      render: (_: unknown, record: FeedLogEntry) => (
        <div style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
          {record.publisher_results.length === 0 ? (
            <Text type="secondary" style={{ fontSize: 11 }}>No publishers</Text>
          ) : (
            record.publisher_results.map((r, i) => (
              <Tooltip key={i} title={r.message}>
                <Tag
                  color={r.success ? "green" : "red"}
                  style={{ fontSize: 11, cursor: "default" }}
                >
                  {r.success ? (
                    <CheckCircleOutlined style={{ marginRight: 2 }} />
                  ) : (
                    <CloseCircleOutlined style={{ marginRight: 2 }} />
                  )}
                  {r.publisher_id}
                </Tag>
              </Tooltip>
            ))
          )}
        </div>
      ),
    },
  ];

  return (
    <div>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 8 }}>
        <Text strong>Publication History</Text>
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          <Text type="secondary" style={{ fontSize: 12 }}>Retention (days):</Text>
          <InputNumber
            size="small"
            min={1}
            max={365}
            value={retentionDays}
            onChange={(v) => setRetentionDays(v ?? 30)}
            style={{ width: 70 }}
          />
          <Button size="small" icon={<SaveOutlined />} onClick={handleSaveRetention} loading={savingRetention}>
            Save
          </Button>
          <Button size="small" icon={<ReloadOutlined />} onClick={loadData}>
            Refresh
          </Button>
        </div>
      </div>

      {data && (
        <Text type="secondary" style={{ fontSize: 12, display: "block", marginBottom: 8 }}>
          {data.total} entries total — automatically cleaned up after {data.retention_days} days
        </Text>
      )}

      <Table
        dataSource={data?.entries ?? []}
        columns={columns}
        rowKey="guid"
        loading={loading}
        size="small"
        pagination={{ pageSize: 20, showSizeChanger: false }}
        locale={{ emptyText: <Empty description="No publication history yet" /> }}
      />
    </div>
  );
}

// ───── Main LogsPage ─────

export default function LogsPage() {
  return (
    <div className="fade-in-up">
      <Title level={3} style={{ margin: 0, marginBottom: 16 }}>
        Logs
      </Title>

      <Tabs
        defaultActiveKey="history"
        items={[
          {
            key: "history",
            label: (
              <span>
                <Badge status="processing" /> History
              </span>
            ),
            children: <FeedLogs />,
          },
          {
            key: "realtime",
            label: (
              <span>
                <Badge status="success" /> Real-time
              </span>
            ),
            children: <SseLogs />,
          },
        ]}
      />
    </div>
  );
}