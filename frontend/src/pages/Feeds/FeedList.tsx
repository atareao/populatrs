import { useEffect, useState } from "react";
import {
  Table, Button, Modal, Form, Input, Select, Switch, Typography, Space, Tag, Tabs, Row, Col, message, Popconfirm,
} from "antd";
import { PlusOutlined, EditOutlined, DeleteOutlined, PlayCircleOutlined, LoadingOutlined } from "@ant-design/icons";
import { fetchFeeds, createFeed, updateFeed, deleteFeed, runFeed, dryRunFeed, fetchPublishers, resolveYoutubeUrl, type FeedConfig, type FeedPublisherBinding } from "../../api/http";

const { Title, Text } = Typography;

function generateId(name: string): string {
  return name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}

export default function FeedList() {
  const [feeds, setFeeds] = useState<FeedConfig[]>([]);
  const [publishers, setPublishers] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [runningFeeds, setRunningFeeds] = useState<Set<string>>(new Set());
  const [modalOpen, setModalOpen] = useState(false);
  const [editingFeed, setEditingFeed] = useState<FeedConfig | null>(null);
  const [selectedPublishers, setSelectedPublishers] = useState<string[]>([]);
  const [feedType, setFeedType] = useState<string>("Rss");
  const [resolveModalOpen, setResolveModalOpen] = useState(false);
  const [resolveUrl, setResolveUrl] = useState("");
  const [resolving, setResolving] = useState(false);
  const [resolveError, setResolveError] = useState<string | null>(null);
  const [runResultVisible, setRunResultVisible] = useState(false);
  const [runResult, setRunResult] = useState<{ success: boolean; feedId: string; feedName: string; postsCount: number; posts: { guid: string; title: string; url: string }[]; message: string } | null>(null);
  const [pendingRun, setPendingRun] = useState<{ id: string; name: string } | null>(null);
  const [publishWithRun, setPublishWithRun] = useState(true);
  const [running, setRunning] = useState(false);
  const [dryRunning, setDryRunning] = useState(false);
  const [form] = Form.useForm();

  const loadData = async () => {
    try {
      const [feedData, pubData] = await Promise.all([fetchFeeds(), fetchPublishers()]);
      setFeeds(feedData.feeds);
      setPublishers(Object.keys(pubData.publishers));
    } catch (e) {
      message.error("Failed to load feeds");
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { loadData(); }, []);

  const handleCreate = () => {
    setEditingFeed(null);
    setSelectedPublishers([]);
    setFeedType("Rss");
    form.resetFields();
    form.setFieldsValue({ enabled: true, type: "Rss" });
    setModalOpen(true);
  };

  const handleEdit = (feed: FeedConfig) => {
    setEditingFeed(feed);
    const ids = feed.publishers.map((b) => b.publisher_id);
    setSelectedPublishers(ids);
    setFeedType(feed.type);
    form.resetFields();
    form.setFieldsValue({ ...feed, publishers: ids });
    // Set per-publisher template overrides
    for (const binding of feed.publishers) {
      if (binding.template) {
        form.setFieldValue(`template_${binding.publisher_id}`, binding.template);
      }
    }
    setModalOpen(true);
  };

  const handleDelete = async (id: string) => {
    try {
      await deleteFeed(id);
      message.success("Feed deleted");
      loadData();
    } catch (e) {
      message.error("Failed to delete feed");
    }
  };

  const handleRun = (id: string, name: string) => {
    setPendingRun({ id, name });
    setPublishWithRun(true);
    setRunResult(null);
    setRunning(false);
    setRunResultVisible(true);
  };

  const handleDryRun = async () => {
    if (!pendingRun) return;
    const { id, name } = pendingRun;
    setDryRunning(true);
    setRunningFeeds((prev) => new Set(prev).add(id));
    try {
      const result = await dryRunFeed(id);
      setRunResult({
        success: true,
        feedId: id,
        feedName: name,
        postsCount: result.posts_count,
        posts: result.posts ?? [],
        message: result.message,
      });
    } catch (e) {
      const msg = e instanceof Error ? e.message : "Unknown error";
      setRunResult({ success: false, feedId: id, feedName: name, postsCount: 0, posts: [], message: `Dry run failed: ${msg}` });
    } finally {
      setDryRunning(false);
      setRunningFeeds((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    }
  };

  const handleRunConfirmed = async () => {
    if (!pendingRun) return;
    const { id, name } = pendingRun;
    setRunning(true);
    setRunningFeeds((prev) => new Set(prev).add(id));
    try {
      const result = await runFeed(id, publishWithRun);
      if (publishWithRun) {
        loadData();
      }
      setRunResult({
        success: true,
        feedId: id,
        feedName: name,
        postsCount: result.posts_count,
        posts: result.posts ?? [],
        message: result.posts_count > 0
          ? publishWithRun
            ? `🚀 Published ${result.posts_count} post(s) to ${name}`
            : `📝 Marked ${result.posts_count} post(s) as seen in ${name}`
          : `No new posts in ${name}`,
      });
    } catch (e) {
      const msg = e instanceof Error ? e.message : "Unknown error";
      setRunResult({ success: false, feedId: id, feedName: name, postsCount: 0, posts: [], message: `Failed: ${msg}` });
    } finally {
      setRunning(false);
      setRunningFeeds((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    }
  };

  const resetRunModal = () => {
    setRunResultVisible(false);
    setRunResult(null);
    setPendingRun(null);
    setRunning(false);
    setDryRunning(false);
  };

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      const pubIds: string[] = values.publishers ?? [];
      // Build FeedPublisherBinding[] from selected IDs and template overrides
      const bindings: FeedPublisherBinding[] = pubIds.map((id: string) => {
        const tpl = form.getFieldValue(`template_${id}`);
        return {
          publisher_id: id,
          template: tpl && tpl.trim() ? tpl.trim() : null,
        };
      });
      values.publishers = bindings;
      // Strip template_ helper fields before sending
      for (const key of Object.keys(values)) {
        if (key.startsWith("template_")) delete (values as Record<string, unknown>)[key];
      }
      if (editingFeed) {
        await updateFeed(editingFeed.id, { ...editingFeed, ...values });
        message.success("Feed updated");
      } else {
        // Auto-generate ID from name
        values.id = generateId(values.name);
        await createFeed(values);
        message.success("Feed created");
      }
      setModalOpen(false);
      loadData();
    } catch (e) {
      if (e instanceof Error) message.error(e.message);
    }
  };

  const columns = [
    { title: "Name", dataIndex: "name", key: "name" },
    { title: "Type", dataIndex: "type", key: "type", render: (t: string) => <Tag>{t}</Tag> },
    {
      title: "Enabled", dataIndex: "enabled", key: "enabled",
      render: (enabled: boolean) => (
        <span>{enabled ? "✅" : "❌"}</span>
      ),
    },
    {
      title: "Publishers", dataIndex: "publishers", key: "publishers",
      render: (pubs: FeedPublisherBinding[]) => pubs.map(p => <Tag key={p.publisher_id}>{p.publisher_id}</Tag>),
    },
    {
      title: "Actions", key: "actions",
      render: (_: unknown, record: FeedConfig) => (
        <Space>
          <Button size="small" icon={<EditOutlined />} onClick={() => handleEdit(record)} />
          <Button
            size="small"
            icon={runningFeeds.has(record.id) ? <LoadingOutlined /> : <PlayCircleOutlined />}
            onClick={() => handleRun(record.id, record.name)}
            loading={runningFeeds.has(record.id)}
          >
            {runningFeeds.has(record.id) ? "Running..." : "Run"}
          </Button>
          <Popconfirm title="Delete this feed?" onConfirm={() => handleDelete(record.id)}>
            <Button size="small" danger icon={<DeleteOutlined />} />
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <div>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 16 }}>
        <Title level={3}>Feeds</Title>
        <Button type="primary" icon={<PlusOutlined />} onClick={handleCreate}>Add Feed</Button>
      </div>
      <Table dataSource={feeds} columns={columns} rowKey="id" loading={loading} />

      <Modal
        title={editingFeed ? "Edit Feed" : "Create Feed"}
        open={modalOpen}
        onOk={handleSubmit}
        onCancel={() => setModalOpen(false)}
        width={600}
      >
        <Form form={form} layout="vertical">
          <Row gutter={16}>
            <Col span={12}>
              <Form.Item name="name" label="Name" rules={[{ required: true, message: "Name is required" }]}>
                <Input placeholder="my-cool-blog" />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item name="type" label="Type" rules={[{ required: true }]}>
                <Select onChange={(value: string) => setFeedType(value)}>
                  <Select.Option value="Rss">RSS</Select.Option>
                  <Select.Option value="Youtube">YouTube</Select.Option>
                </Select>
              </Form.Item>
            </Col>
          </Row>
          <Row gutter={16}>
            <Col span={12}>
              {feedType === "Rss" && (
                <Form.Item name={["config", "url"]} label="URL (RSS)" rules={[{ required: true, message: "RSS URL is required" }]}>
                  <Input placeholder="https://example.com/feed.xml" />
                </Form.Item>
              )}
              {feedType === "Youtube" && (
                <Form.Item name={["config", "channel_id"]} label="Channel ID (YouTube)" rules={[{ required: true, message: "Channel ID is required" }]}>
                  <Input
                    placeholder="UC..."
                    addonAfter={
                      <a onClick={() => { setResolveUrl(""); setResolveError(null); setResolveModalOpen(true); }} style={{ cursor: "pointer" }}>
                        Resolve from URL
                      </a>
                    }
                  />
                </Form.Item>
              )}
            </Col>
            <Col span={12}>
              <Form.Item name="enabled" label="Enabled" valuePropName="checked">
                <Switch />
              </Form.Item>
            </Col>
          </Row>
          <Form.Item name="publishers" label="Publishers">
            <Select
              mode="multiple"
              placeholder="Select publishers"
              onChange={(ids: string[]) => setSelectedPublishers(ids)}
            >
              {publishers.map(p => <Select.Option key={p} value={p}>{p}</Select.Option>)}
            </Select>
          </Form.Item>
          {selectedPublishers.length > 0 && (
            <Tabs
              items={selectedPublishers.map(id => ({
                key: id,
                label: id,
                children: (
                  <Form.Item key={id} name={`template_${id}`} label={`Template override (${id})`}>
                    <Input.TextArea rows={4} placeholder="Leave empty to use publisher's default template" />
                  </Form.Item>
                ),
              }))}
            />
          )}
        </Form>
      </Modal>
      <Modal
        title={runResult ? (runResult.success ? `✅ ${runResult.feedName}` : `❌ ${runResult.feedName}`) : `Run ${pendingRun?.name ?? ""}`}
        open={runResultVisible}
        onCancel={resetRunModal}
        footer={runResult
          ? <Space>
              <Button onClick={resetRunModal}>Close</Button>
              {runResult.postsCount > 0 && runResult.posts && (
                <Button onClick={() => setRunResult(null)}>Back</Button>
              )}
            </Space>
          : <Space>
              <Button onClick={resetRunModal}>Cancel</Button>
              <Button onClick={handleDryRun} loading={dryRunning}>
                {dryRunning ? "Dry running..." : "🔍 Dry Run"}
              </Button>
              <Button type="primary" onClick={handleRunConfirmed} loading={running}>
                {running ? "Running..." : "Execute"}
              </Button>
            </Space>
        }
      >
        {runResult ? (
          <>
            <p>{runResult.message}</p>
            {runResult.posts && runResult.posts.length > 0 && (
              <ul style={{ marginTop: 8 }}>
                {runResult.posts.map((p) => (
                  <li key={p.guid}>
                    <a href={p.url} target="_blank" rel="noopener noreferrer">{p.title}</a>
                  </li>
                ))}
              </ul>
            )}
          </>
        ) : (
          <>
            <p>Run feed <strong>{pendingRun?.name}</strong>?</p>
            <div style={{ marginTop: 12 }}>
              <Space>
                <Switch checked={publishWithRun} onChange={(v) => setPublishWithRun(v)} />
                <span>Publish to publishers</span>
              </Space>
            </div>
            <p style={{ color: "#888", fontSize: 12, marginTop: 8 }}>
              {publishWithRun
                ? "Posts will be published and appear in history with publisher results."
                : "Posts will be marked as seen and appear in history with 'No publishers'."}
            </p>
          </>
        )}
      </Modal>
      <Modal
        title="Resolve YouTube URL"
        open={resolveModalOpen}
        onCancel={() => setResolveModalOpen(false)}
        onOk={async () => {
          const url = resolveUrl.trim();
          if (!url) return;
          setResolveError(null);
          // Basic frontend validation — must be a YouTube URL or a bare handle/channel ID
          if (url.startsWith("http") && !url.includes("youtube.com") && !url.includes("youtu.be")) {
            setResolveError("Not a YouTube URL. Paste a youtube.com link or a @handle / UCxxx.");
            return;
          }
          setResolving(true);
          try {
            const result = await resolveYoutubeUrl(url);
            form.setFieldValue(["config", "channel_id"], result.channel_id);
            setResolveModalOpen(false);
            message.success("Channel ID resolved successfully");
          } catch (e) {
            const msg = e instanceof Error ? e.message : "Unknown error";
            setResolveError(msg);
          } finally {
            setResolving(false);
          }
        }}
        confirmLoading={resolving}
      >
        <p>Paste a YouTube channel URL or handle:</p>
        <Input
          placeholder="https://youtube.com/@channel or UCxxxxx"
          value={resolveUrl}
          onChange={(e) => { setResolveUrl(e.target.value); setResolveError(null); }}
          onPressEnter={() => {
            const url = resolveUrl.trim();
            if (!url) return;
            setResolveError(null);
            if (url.startsWith("http") && !url.includes("youtube.com") && !url.includes("youtu.be")) {
              setResolveError("Not a YouTube URL. Paste a youtube.com link or a @handle / UCxxx.");
              return;
            }
            (async () => {
              setResolving(true);
              try {
                const result = await resolveYoutubeUrl(url);
                form.setFieldValue(["config", "channel_id"], result.channel_id);
                setResolveModalOpen(false);
                message.success("Channel ID resolved successfully");
              } catch (e) {
                const msg = e instanceof Error ? e.message : "Unknown error";
                setResolveError(msg);
              } finally {
                setResolving(false);
              }
            })();
          }}
        />
        {resolveError && <p style={{ color: "red", marginTop: 8 }}>❌ {resolveError}</p>}
      </Modal>
    </div>
  );
}