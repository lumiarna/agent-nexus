import { useEffect, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, Dot, Input } from "@/components/ui/primitives";
import { ScreenScroll } from "@/components/shell/screen";
import { useNav } from "@/lib/nav";
import { nexus } from "@/lib/mock";
import {
  useSaveWebdavSettingsMutation,
  useTestWebdavConnectionMutation,
  useWebdavSettingsQuery,
} from "@/lib/query/sync";
import { fallbackAgentCapabilities } from "@/lib/agentCapabilities";
import { useAgentCapabilitiesQuery } from "@/lib/query/agentCapabilities";

type WebdavStatus = "ok" | "testing" | "untested";

const WS_INFO: Record<WebdavStatus, { label: string; fg: string; dot: string }> = {
  ok: { label: "Connected", fg: "#5f7a3e", dot: "#8a9a5b" },
  testing: { label: "Testing…", fg: "#9d7a64", dot: "#9d7a64" },
  untested: { label: "Not tested", fg: "#a99a89", dot: "#d9c9b3" },
};

function getErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  if (
    error &&
    typeof error === "object" &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }
  return "Unexpected error";
}

export function SettingsPage() {
  const { go } = useNav();
  const webdavSettingsQuery = useWebdavSettingsQuery();
  const agentCapabilitiesQuery = useAgentCapabilitiesQuery();
  const saveWebdavSettingsMutation = useSaveWebdavSettingsMutation();
  const testWebdavConnectionMutation = useTestWebdavConnectionMutation();
  const [init] = useState(() => nexus.settings());
  const [url, setUrl] = useState(init.webdav.url);
  const [user, setUser] = useState(init.webdav.user);
  const [pass, setPass] = useState(init.webdav.pass);
  const [remoteRoot, setRemoteRoot] = useState(init.webdav.remoteRoot);
  const [webdavStatus, setWebdavStatus] = useState<WebdavStatus>(
    init.webdav.status === "ok" ? "ok" : "untested",
  );
  const agents = agentCapabilitiesQuery.data ?? fallbackAgentCapabilities();

  const ws = WS_INFO[webdavStatus];

  useEffect(() => {
    if (!webdavSettingsQuery.data) return;
    setUrl(webdavSettingsQuery.data.url);
    setUser(webdavSettingsQuery.data.user);
    setPass(webdavSettingsQuery.data.pass);
    setRemoteRoot(webdavSettingsQuery.data.remoteRoot);
    setWebdavStatus(webdavSettingsQuery.data.status === "ok" ? "ok" : "untested");
  }, [webdavSettingsQuery.data]);

  async function testWebdav() {
    setWebdavStatus("testing");
    try {
      await testWebdavConnectionMutation.mutateAsync({
        url,
        user,
        pass,
        remoteRoot,
      });
      setWebdavStatus("ok");
      toast("WebDAV connection ok");
    } catch (error) {
      setWebdavStatus("untested");
      toast(getErrorMessage(error));
    }
  }

  async function saveWebdav() {
    try {
      const saved = await saveWebdavSettingsMutation.mutateAsync({
        url,
        user,
        pass,
        remoteRoot,
      });
      setUrl(saved.url);
      setUser(saved.user);
      setPass(saved.pass);
      setRemoteRoot(saved.remoteRoot);
      setWebdavStatus("untested");
      toast("WebDAV settings saved");
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  return (
    <ScreenScroll maxWidth="1040px">
      <button
        onClick={() => go("provider")}
        className="mb-3 inline-flex items-center gap-1.5 text-[12px] text-[#9a8f80] hover:text-nexus-accent"
      >
        ← Back to app
      </button>
      <h1 className="m-0 text-[23px] font-extrabold tracking-[-.02em] text-nexus-ink">Settings</h1>
      <p className="mt-1.5 text-[13px] text-[#9a8f80]">
        Global configuration · WebDAV, taskbar surface, and agent config roots
      </p>

      {/* WebDAV */}
      <Card className="mt-[22px] p-[22px]">
        <div className="flex flex-wrap items-center gap-2.5">
          <h2 className="m-0 text-[15px] font-extrabold text-nexus-ink">WebDAV</h2>
          <span className="text-[11px] text-[#b3a999]">
            Cloud destination for Session backup &amp; aggregation
          </span>
          <span
            className="ml-auto inline-flex items-center gap-1.5 text-[11.5px] font-bold"
            style={{ color: ws.fg }}
          >
            <Dot color={ws.dot} /> {ws.label}
          </span>
        </div>
        <div className="mt-4 grid grid-cols-2 gap-3.5">
          <label className="block">
            <div className="mb-1.5 text-[12px] font-semibold text-[#6a6055]">Endpoint URL</div>
            <Input
              className="font-mono"
              placeholder="https://nas.local/webdav/agent-nexus"
              value={url}
              onChange={(e) => {
                setUrl(e.target.value);
                setWebdavStatus("untested");
              }}
            />
          </label>
          <label className="block">
            <div className="mb-1.5 text-[12px] font-semibold text-[#6a6055]">
              Remote root directory
            </div>
            <Input
              className="font-mono"
              placeholder="agent-nexus-sync"
              value={remoteRoot}
              onChange={(e) => {
                setRemoteRoot(e.target.value);
                setWebdavStatus("untested");
              }}
            />
          </label>
          <label className="block">
            <div className="mb-1.5 text-[12px] font-semibold text-[#6a6055]">Username</div>
            <Input
              className="font-mono"
              placeholder="nexus"
              value={user}
              onChange={(e) => {
                setUser(e.target.value);
                setWebdavStatus("untested");
              }}
            />
          </label>
          <label className="block">
            <div className="mb-1.5 text-[12px] font-semibold text-[#6a6055]">
              Password / App token
            </div>
            <Input
              className="font-mono"
              type="password"
              placeholder="••••••••"
              value={pass}
              onChange={(e) => {
                setPass(e.target.value);
                setWebdavStatus("untested");
              }}
            />
          </label>
        </div>
        <div className="mt-4 flex flex-wrap items-center gap-[9px]">
          <Button
            variant="primary"
            size="md"
            className="px-4"
            onClick={() => void testWebdav()}
            disabled={testWebdavConnectionMutation.isPending}
          >
            {testWebdavConnectionMutation.isPending ? "Testing..." : "Test connection"}
          </Button>
          <Button
            variant="subtle"
            size="md"
            className="px-4"
            onClick={() => void saveWebdav()}
            disabled={saveWebdavSettingsMutation.isPending}
          >
            {saveWebdavSettingsMutation.isPending ? "Saving..." : "Save"}
          </Button>
          <span className="text-[11px] text-[#c3b9a8]">
            Archive layout:{" "}
            <span className="font-mono">&lt;endpoint&gt;/&lt;remote root&gt;/&lt;task path&gt;</span>{" "}
            · shown as <b className="text-[#9a8f80]">Cloud</b> in the app
          </span>
        </div>
      </Card>

      {/* Agent config roots */}
      <Card className="mt-4 p-[22px]">
        <div className="flex flex-wrap items-center gap-2.5">
          <h2 className="m-0 text-[15px] font-extrabold text-nexus-ink">Agent config roots</h2>
          <span className="text-[11px] text-[#b3a999]">
            Where Skill &amp; Prompt placements are written · backend capability order
          </span>
        </div>
        <div className="mt-4 flex flex-col gap-3">
          {agents.map((a) => (
            <div
              key={a.name}
              className="rounded-[14px] border border-nexus-panel bg-nexus-sand2 px-4 py-3.5"
            >
              <div className="flex items-center gap-[9px]">
                <span
                  className="inline-flex h-6 w-6 items-center justify-center rounded-[7px] text-[9px] font-extrabold text-white"
                  style={{ background: a.color }}
                >
                  {a.abbr}
                </span>
                <span className="text-[13.5px] font-bold text-nexus-ink">{a.name}</span>

              </div>
              <div className="mt-[11px] grid grid-cols-4 gap-3">
                {agentDirs(a).map((d) => (
                  <div key={d.key}>
                    <div className="font-mono text-[9.5px] font-semibold tracking-[.04em] text-[#c3b9a8]">
                      {d.key}
                    </div>
                    <div className="mt-[3px] overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11.5px] text-[#6a6055]">
                      {d.value}
                    </div>
                    {d.derivedFrom ? (
                      <div className="mt-0.5 text-[9.5px] text-[#bca37a]">derived from {d.derivedFrom}</div>
                    ) : null}
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      </Card>
    </ScreenScroll>
  );
}

function agentDirs(agent: ReturnType<typeof fallbackAgentCapabilities>[number]) {
  return [
    { key: "CONFIG_ROOT", value: agent.configDir },
    agent.skill
      ? { key: "GLOBAL_SKILLS", value: agent.skill.globalDir, derivedFrom: "CONFIG_ROOT" }
      : null,
    agent.skill ? { key: "PROJECT_SKILLS", value: agent.skill.projectDir } : null,
    agent.prompt
      ? { key: "GLOBAL_PROMPT", value: agent.prompt.globalFile, derivedFrom: "CONFIG_ROOT" }
      : null,
  ].filter((dir): dir is { key: string; value: string; derivedFrom?: string } => dir !== null);
}
