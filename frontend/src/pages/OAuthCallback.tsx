import { useEffect, useState } from "react";
import { useSearchParams } from "react-router";
import { Typography, Spin, Result, Button } from "antd";
import { oauthCallback } from "../api/http";

const { Text } = Typography;

export default function OAuthCallback() {
  const [searchParams] = useSearchParams();
  const [status, setStatus] = useState<"processing" | "success" | "error">("processing");
  const [message, setMessage] = useState("");

  const code = searchParams.get("code");
  const state = searchParams.get("state");
  const publisherId = searchParams.get("publisher_id");

  useEffect(() => {
    if (!code || !state || !publisherId) {
      setStatus("error");
      setMessage("Missing OAuth parameters (code, state, or publisher_id)");
      return;
    }

    oauthCallback(publisherId, code, state)
      .then((result) => {
        setStatus("success");
        setMessage(result.message);
        // Auto-close after 2 seconds
        setTimeout(() => window.close(), 2000);
      })
      .catch((e) => {
        setStatus("error");
        setMessage(e instanceof Error ? e.message : "OAuth callback failed");
      });
  }, [code, state, publisherId]);

  return (
    <div
      style={{
        display: "flex",
        justifyContent: "center",
        alignItems: "center",
        minHeight: "100vh",
        background: "#0f0f1a",
      }}
    >
      {status === "processing" && (
        <div style={{ textAlign: "center" }}>
          <Spin size="large" />
          <br />
          <Text style={{ color: "#e0e0e0", marginTop: 16, display: "block" }}>
            Completing OAuth authentication...
          </Text>
        </div>
      )}
      {status === "success" && (
        <Result
          status="success"
          title="Connected!"
          subTitle={message}
          extra={
            <Button type="primary" onClick={() => window.close()}>
              Close window
            </Button>
          }
        />
      )}
      {status === "error" && (
        <Result
          status="error"
          title="Connection failed"
          subTitle={message}
          extra={
            <Button type="primary" onClick={() => window.close()}>
              Close window
            </Button>
          }
        />
      )}
    </div>
  );
}