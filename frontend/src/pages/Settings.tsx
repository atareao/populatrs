import { useEffect, useState } from "react"
import {
  Card,
  Form,
  Input,
  InputNumber,
  Select,
  Button,
  Typography,
  message,
  Spin,
  Alert,
  Tag,
  Tabs,
  type TabsProps,
} from "antd"
import {
  SaveOutlined,
  InfoCircleOutlined,
  YoutubeOutlined,
  ClockCircleOutlined,
  GlobalOutlined,
  LinkOutlined,
  ReloadOutlined,
} from "@ant-design/icons"
import {
  fetchYoutubeConfig,
  updateYoutubeConfig,
  fetchSchedule,
  updateSchedule,
  fetchRetryPolicy,
  updateRetryPolicy,
  fetchPublishSettings,
  updatePublishSettings,
  type ScheduleConfig,
  type RetryPolicy,
  type PublishSettings,
} from "../api/http"

const { Title } = Typography

export const CRON_PRESETS = [
  { label: "Every 5 minutes", value: "*/5 * * * *" },
  { label: "Every 15 minutes", value: "*/15 * * * *" },
  { label: "Every 30 minutes", value: "*/30 * * * *" },
  { label: "Every hour", value: "0 * * * *" },
  { label: "Every 6 hours", value: "0 */6 * * *" },
  { label: "Daily at 06:00", value: "0 6 * * *" },
  { label: "Daily at 06:00, 10:00, 12:00", value: "0 6,10,12 * * *" },
  { label: "Daily at 6:05, 10:05, 18:05", value: "5 6,10,18 * * *" },
  { label: "Daily at 10:05, 18:05", value: "5 10,18 * * *" },
  { label: "Daily at 22:00", value: "0 22 * * *" },
]

export default function Settings() {
  const [loading, setLoading] = useState(true)
  const [activeTab, setActiveTab] = useState("schedule")
  const [ytSaving, setYtSaving] = useState(false)
  const [schedSaving, setSchedSaving] = useState(false)
  const [currentCron, setCurrentCron] = useState<string>("0 * * * *")
  const [currentTimezone, setCurrentTimezone] = useState<string>("UTC")
  const [nextRunAt, setNextRunAt] = useState<string | null>(null)
  const [ytForm] = Form.useForm()
  const [schedForm] = Form.useForm()
  const [retryPolicy, setRetryPolicy] = useState<RetryPolicy | null>(null)
  const [retrySaving, setRetrySaving] = useState(false)
  const [retryForm] = Form.useForm()
  const [publishSettings, setPublishSettings] = useState<PublishSettings | null>(null)
  const [publishSaving, setPublishSaving] = useState(false)
  const [publishForm] = Form.useForm()

  useEffect(() => {
    Promise.all([
      fetchYoutubeConfig().then((d) => ytForm.setFieldsValue(d)),
      fetchSchedule().then((d) => {
        schedForm.setFieldsValue(d)
        setCurrentCron(d.cron_expression)
        setCurrentTimezone(d.timezone)
        setNextRunAt(d.next_run_at ?? null)
      }),
      fetchRetryPolicy().then((d) => {
        retryForm.setFieldsValue(d)
        setRetryPolicy(d)
      }),
      fetchPublishSettings().then((d) => {
        publishForm.setFieldsValue(d)
        setPublishSettings(d)
      }),
    ])
      .catch(() => message.error("Failed to load config"))
      .finally(() => setLoading(false))
  }, [ytForm, schedForm, retryForm, publishForm])

  const handleYtSubmit = async (values: { api_key: string }) => {
    setYtSaving(true)
    try {
      await updateYoutubeConfig(values)
      message.success("YouTube config saved")
    } catch {
      message.error("Failed to save YouTube config")
    } finally {
      setYtSaving(false)
    }
  }

  const handleSchedSubmit = async (values: ScheduleConfig) => {
    setSchedSaving(true)
    try {
      await updateSchedule(values)
      setCurrentCron(values.cron_expression)
      setCurrentTimezone(values.timezone)
      setNextRunAt(values.next_run_at ?? null)
      message.success("Schedule updated")
    } catch {
      message.error("Failed to update schedule")
    } finally {
      setSchedSaving(false)
    }
  }

  const handleRetrySubmit = async (values: RetryPolicy) => {
    setRetrySaving(true)
    try {
      await updateRetryPolicy(values)
      setRetryPolicy(values)
      message.success("Retry policy saved")
    } catch {
      message.error("Failed to save retry policy")
    } finally {
      setRetrySaving(false)
    }
  }

  const handlePublishSubmit = async (values: PublishSettings) => {
    setPublishSaving(true)
    try {
      await updatePublishSettings(values)
      setPublishSettings(values)
      message.success("Publish settings saved")
    } catch {
      message.error("Failed to save publish settings")
    } finally {
      setPublishSaving(false)
    }
  }

  if (loading)
    return (
      <div style={{ textAlign: "center", padding: 40 }}>
        <Spin size="large" data-testid="spinner" />
      </div>
    )

  const tabItems: TabsProps["items"] = [
    {
      key: "schedule",
      label: (
        <span>
          <ClockCircleOutlined /> Schedule
        </span>
      ),
      children: (
        <>
          <Tag icon={<GlobalOutlined />} style={{ marginBottom: 12 }}>
            {currentTimezone} · {currentCron}
          </Tag>
          {nextRunAt && (
            <Tag color="blue" style={{ marginBottom: 12 }}>
              Next run: {new Date(nextRunAt).toLocaleString()}
            </Tag>
          )}
          <Form
            form={schedForm}
            layout="vertical"
            onFinish={handleSchedSubmit}
            initialValues={{ cron_expression: "0 * * * *", timezone: "UTC" }}
          >
            <Form.Item label="Presets" style={{ marginBottom: 8 }}>
              <Select
                allowClear
                placeholder="Select a preset..."
                style={{ width: "100%" }}
                options={CRON_PRESETS}
                onChange={(value) => {
                  if (value) schedForm.setFieldsValue({ cron_expression: value })
                }}
              />
            </Form.Item>
            <Form.Item
              name="cron_expression"
              label="Cron Expression"
              rules={[{ required: true, message: "Enter a cron expression" }]}
              help={
                <span>
                  Five-field cron syntax.{" "}
                  <a
                    href="https://crontab.guru/"
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    <LinkOutlined /> crontab.guru
                  </a>
                </span>
              }
            >
              <Input placeholder="0 */6 * * *" />
            </Form.Item>
            <Form.Item
              name="timezone"
              label="Timezone"
              rules={[{ required: true, message: "Enter a timezone" }]}
              help="e.g. UTC, Europe/Madrid"
            >
              <Input placeholder="UTC" />
            </Form.Item>
            <Button
              type="primary"
              htmlType="submit"
              loading={schedSaving}
              icon={<SaveOutlined />}
            >
              Save Schedule
            </Button>
          </Form>
        </>
      ),
    },
    {
      key: "youtube",
      label: (
        <span>
          <YoutubeOutlined /> YouTube API Key
        </span>
      ),
      children: (
        <>
          <Alert
            message="A YouTube Data API v3 key is needed to fetch videos and resolve @handles to channel IDs. Get one at https://console.cloud.google.com/apis/credentials"
            type="info"
            showIcon
            icon={<InfoCircleOutlined />}
            style={{ marginBottom: 20 }}
          />
          <Form
            form={ytForm}
            layout="vertical"
            onFinish={handleYtSubmit}
            initialValues={{ api_key: "" }}
          >
            <Form.Item
              name="api_key"
              label="YouTube Data API Key"
              rules={[
                {
                  required: true,
                  message: "API key is required for YouTube feeds",
                },
              ]}
            >
              <Input.Password placeholder="AIzaSy..." />
            </Form.Item>
            <Button
              type="primary"
              htmlType="submit"
              loading={ytSaving}
              icon={<SaveOutlined />}
            >
              Save
            </Button>
          </Form>
        </>
      ),
    },
    {
      key: "retry",
      label: (
        <span>
          <ReloadOutlined /> Retry Policy
        </span>
      ),
      children: (
        <>
          <Alert
            message="When a publish attempt fails, populatrs will automatically retry with exponential backoff."
            type="info"
            showIcon
            icon={<InfoCircleOutlined />}
            style={{ marginBottom: 20 }}
          />
          <Form
            form={retryForm}
            layout="vertical"
            onFinish={handleRetrySubmit}
            initialValues={{
              max_retries: 3,
              base_delay_seconds: 5,
              max_delay_seconds: 300,
              backoff_multiplier: 2.0,
            }}
          >
            <Form.Item
              name="max_retries"
              label="Max Retries"
              rules={[{ required: true, message: "Required" }]}
            >
              <InputNumber min={0} max={10} style={{ width: "100%" }} />
            </Form.Item>
            <Form.Item
              name="base_delay_seconds"
              label="Base Delay (seconds)"
              rules={[{ required: true, message: "Required" }]}
            >
              <InputNumber min={1} max={3600} style={{ width: "100%" }} />
            </Form.Item>
            <Form.Item
              name="max_delay_seconds"
              label="Max Delay (seconds)"
              rules={[{ required: true, message: "Required" }]}
            >
              <InputNumber min={1} max={86400} style={{ width: "100%" }} />
            </Form.Item>
            <Form.Item
              name="backoff_multiplier"
              label="Backoff Multiplier"
              rules={[{ required: true, message: "Required" }]}
            >
              <InputNumber min={1} max={10} step={0.1} style={{ width: "100%" }} />
            </Form.Item>
            <Button
              type="primary"
              htmlType="submit"
              loading={retrySaving}
              icon={<SaveOutlined />}
            >
              Save Retry Policy
            </Button>
          </Form>
        </>
      ),
    },
    {
      key: "publish",
      label: (
        <span>
          <InfoCircleOutlined /> Publish Settings
        </span>
      ),
      children: (
        <>
          <Alert
            message="Control how many posts are published per cycle and filter by age."
            type="info"
            showIcon
            icon={<InfoCircleOutlined />}
            style={{ marginBottom: 20 }}
          />
          <Form
            form={publishForm}
            layout="vertical"
            onFinish={handlePublishSubmit}
            initialValues={{ max_posts: 1, min_date: "" }}
          >
            <Form.Item
              name="max_posts"
              label="Max Posts Per Cycle"
              rules={[{ required: true, message: "Required" }]}
              help="Maximum number of posts to publish each cycle. Set to 0 to publish all pending posts."
            >
              <InputNumber min={0} max={100} style={{ width: "100%" }} />
            </Form.Item>
            <Form.Item
              name="min_date"
              label="Minimum Date"
              help="Skip posts older than this date. Format: YYYY-MM-DD or RFC3339 (e.g. 2024-01-15T10:00:00Z). Leave empty for no filter."
            >
              <Input placeholder="2024-01-01" />
            </Form.Item>
            <Button
              type="primary"
              htmlType="submit"
              loading={publishSaving}
              icon={<SaveOutlined />}
            >
              Save Publish Settings
            </Button>
          </Form>
        </>
      ),
    },
  ]

  return (
    <div className="fade-in-up">
      <Title level={3}>
        <YoutubeOutlined /> Settings
      </Title>

      <Card style={{ maxWidth: 700 }}>
        <Tabs
          activeKey={activeTab}
          onChange={setActiveTab}
          items={tabItems}
          tabBarStyle={{ marginBottom: 16 }}
        />
      </Card>
    </div>
  )
}