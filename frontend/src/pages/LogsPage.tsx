import { useEffect, useState } from "react";
import {
  Typography, Tag, Empty, Spin, Table, Tooltip, InputNumber, Button, message,
} from "antd";
import {
  CheckCircleOutlined, CloseCircleOutlined, ReloadOutlined, SaveOutlined,
} from "@ant-design/icons";
import {
  fetchFeedLogs, updateLogRetention, fetchLogRetention,
  type FeedLogEntry, type FeedLogResponse,
} from "../api/http";

const { Text, Title } = Typography;

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

export default function LogsPage() {
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
    <div className="fade-in-up">
      <Title level={3} style={{ margin: 0, marginBottom: 16 }}>
        Publication History
      </Title>

      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 8 }}>
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
        rowKey={(record) => `${record.guid}-${record.feed_id}`}
        loading={loading}
        size="small"
        pagination={{ pageSize: 20, showSizeChanger: false }}
        locale={{ emptyText: <Empty description="No publication history yet" /> }}
      />
    </div>
  );
}