import { useEffect, useState } from "react";
import {
  Table, Button, Modal, Form, Input, Select, Switch, Typography, Space, Tag, message, Popconfirm,
} from "antd";
import { PlusOutlined, EditOutlined, DeleteOutlined, PlayCircleOutlined, LoadingOutlined } from "@ant-design/icons";
import { fetchFeeds, createFeed, updateFeed, deleteFeed, runFeed, fetchPublishers, type FeedConfig, type FeedPublisherBinding } from "../../api/http";

const { Title, Text } = Typography;

export default function FeedList() {
  const [feeds, setFeeds] = useState<FeedConfig[]>([]);
  const [publishers, setPublishers] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [runningFeeds, setRunningFeeds] = useState<Set<string>>(new Set());
  const [modalOpen, setModalOpen] = useState(false);
  const [editingFeed, setEditingFeed] = useState<FeedConfig | null>(null);
  const [selectedPublishers, setSelectedPublishers] = useState<string[]>([]);
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
    form.resetFields();
    form.setFieldsValue({ enabled: true, type: "Rss" });
    setModalOpen(true);
  };

  const handleEdit = (feed: FeedConfig) => {
    setEditingFeed(feed);
    const ids = feed.publishers.map((b) => b.publisher_id);
    setSelectedPublishers(ids);
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

  const handleRun = async (id: string, name: string) => {
    setRunningFeeds((prev) => new Set(prev).add(id));
    try {
      const result = await runFeed(id);
      if (result.posts_count > 0) {
        message.success({ content: `Found ${result.posts_count} new post(s) in ${name}`, key: `run-${id}`, duration: 5 });
      } else {
        message.info({ content: `No new posts in ${name}`, key: `run-${id}`, duration: 4 });
      }
    } catch (e) {
      message.error({ content: `Failed to run ${name}: ${e instanceof Error ? e.message : "Unknown error"}`, key: `run-${id}`, duration: 6 });
    } finally {
      setRunningFeeds((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    }
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
          <Form.Item name="id" label="Feed ID" rules={[{ required: true }]}>
            <Input disabled={!!editingFeed} />
          </Form.Item>
          <Form.Item name="name" label="Name" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item name="type" label="Type" rules={[{ required: true }]}>
            <Select>
              <Select.Option value="Rss">RSS</Select.Option>
              <Select.Option value="Youtube">YouTube</Select.Option>
            </Select>
          </Form.Item>
          <Form.Item name={["config", "url"]} label="URL (RSS)">
            <Input placeholder="https://example.com/feed.xml" />
          </Form.Item>
          <Form.Item name={["config", "channel_id"]} label="Channel ID (YouTube)">
            <Input placeholder="UC..." />
          </Form.Item>
          <Form.Item name="enabled" label="Enabled" valuePropName="checked">
            <Switch />
          </Form.Item>
          <Form.Item name="publishers" label="Publishers">
            <Select
              mode="multiple"
              placeholder="Select publishers"
              onChange={(ids: string[]) => setSelectedPublishers(ids)}
            >
              {publishers.map(p => <Select.Option key={p} value={p}>{p}</Select.Option>)}
            </Select>
          </Form.Item>
          {selectedPublishers.map(id => (
            <Form.Item key={id} name={`template_${id}`} label={`${id} template`}>
              <Input.TextArea rows={2} placeholder="Leave empty to use publisher's default template" />
            </Form.Item>
          ))}
        </Form>
      </Modal>
    </div>
  );
}