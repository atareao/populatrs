import { useEffect, useState } from "react";
import {
  Table, Button, Modal, Form, Input, Select, Switch, Typography, Space, Tag, message, Popconfirm,
} from "antd";
import { PlusOutlined, EditOutlined, DeleteOutlined, PlayCircleOutlined } from "@ant-design/icons";
import { fetchFeeds, createFeed, updateFeed, deleteFeed, toggleFeed, runFeed, fetchPublishers, type FeedConfig } from "../../api/http";

const { Title } = Typography;

export default function FeedList() {
  const [feeds, setFeeds] = useState<FeedConfig[]>([]);
  const [publishers, setPublishers] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [modalOpen, setModalOpen] = useState(false);
  const [editingFeed, setEditingFeed] = useState<FeedConfig | null>(null);
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
    form.resetFields();
    form.setFieldsValue({ enabled: true, type: "Rss" });
    setModalOpen(true);
  };

  const handleEdit = (feed: FeedConfig) => {
    setEditingFeed(feed);
    form.setFieldsValue(feed);
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

  const handleToggle = async (id: string) => {
    try {
      await toggleFeed(id);
      loadData();
    } catch (e) {
      message.error("Failed to toggle feed");
    }
  };

  const handleRun = async (id: string, name: string) => {
    try {
      message.loading({ content: `Running ${name}...`, key: `run-${id}` });
      const result = await runFeed(id);
      if (result.posts_count > 0) {
        message.success({ content: `Found ${result.posts_count} new post(s) in ${name}`, key: `run-${id}` });
      } else {
        message.info({ content: `No new posts in ${name}`, key: `run-${id}` });
      }
    } catch (e) {
      message.error({ content: `Failed to run ${name}: ${e instanceof Error ? e.message : "Unknown error"}`, key: `run-${id}` });
    }
  };

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      values.publishers ??= [];
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
    { title: "ID", dataIndex: "id", key: "id", ellipsis: true },
    {
      title: "Enabled", dataIndex: "enabled", key: "enabled",
      render: (_: boolean, record: FeedConfig) => (
        <Switch checked={record.enabled} onChange={() => handleToggle(record.id)} />
      ),
    },
    {
      title: "Publishers", dataIndex: "publishers", key: "publishers",
      render: (pubs: string[]) => pubs.map(p => <Tag key={p}>{p}</Tag>),
    },
    {
      title: "Actions", key: "actions",
      render: (_: unknown, record: FeedConfig) => (
        <Space>
          <Button size="small" icon={<EditOutlined />} onClick={() => handleEdit(record)} />
          <Button size="small" icon={<PlayCircleOutlined />} onClick={() => handleRun(record.id, record.name)}>
            Run
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
            <Select mode="multiple" placeholder="Select publishers">
              {publishers.map(p => <Select.Option key={p} value={p}>{p}</Select.Option>)}
            </Select>
          </Form.Item>
          <Form.Item name="check_interval_minutes" label="Check Interval (minutes)">
            <Input type="number" />
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}