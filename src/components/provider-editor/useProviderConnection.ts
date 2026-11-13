import { errorText, uiMessage } from "../../i18n/errors";
import { useMessageState } from "../../i18n/use-message-state";
import { useCallback, useEffect, useRef, useState } from "react";
import { fetchProviderModels, getGatewayStatus, resolveProviderEndpoints,
  type AppKind, type ProviderEndpoints, type ProviderModel,
  type ProviderConnectionOptions, type ResponsesOptions, type UpstreamProtocol } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { NATIVE_PROTOCOL, protocolLabel, requiresGateway, type GatewayProfileInput } from "../../lib/protocol";

/** The routing facts every editor needs; both provider contracts project onto it. */
export interface ProviderConnectionInput extends GatewayProfileInput {
  app: AppKind;
  routeMode: "official" | "custom";
  baseUrl: string | null;
  connection?: ProviderConnectionOptions | null;
  apiKey: string;
  authentication?: import("../../api/shared").AuthenticationScheme | null;
  upstreamProtocol: UpstreamProtocol | null;
  responsesOptions: ResponsesOptions | null;
}

function useGatewayWarning(input: ProviderConnectionInput) {
  const { t } = useI18n();
  const [address, setAddress] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const routed = requiresGateway(input);
  useEffect(() => {
    let active = true;
    setAddress(null);
    setFailed(false);
    if (routed) {
      void getGatewayStatus().then((status) => {
        if (!active) return;
        setAddress(status.baseUrl);
        setFailed(status.baseUrl === null);
      }).catch(() => { if (active) setFailed(true); });
    }
    return () => { active = false; };
  }, [routed]);
  const minimal = input.upstreamProtocol === "responses" && input.responsesOptions?.requestMode === "minimal";
  const lead = input.app === "codex" ? t("providers.gateway.lead.official") : minimal ? t("providers.gateway.lead.minimal")
    : t("providers.gateway.lead.protocol", { client: clientName(input.app), protocol: protocolLabel(t, NATIVE_PROTOCOL[input.app]) });
  const action = minimal ? t("providers.gateway.actionMinimal")
    : input.upstreamProtocol === NATIVE_PROTOCOL[input.app] ? t("providers.gateway.actionNative") : t("providers.gateway.actionConverted");
  const tail = address
    ? t("providers.gateway.tail.routed", { address, action })
    : failed
      ? t("providers.gateway.tail.failed")
      : t("providers.gateway.tail.pending");
  return `${lead}${tail}`;
}

function useResolvedEndpoints(
  baseUrl: string,
  upstreamProtocol: UpstreamProtocol | null,
  connection: ProviderConnectionOptions | null | undefined,
) {
  const { t } = useI18n();
  const [resolution, setResolution] = useState<{
    baseUrl: string;
    upstreamProtocol: UpstreamProtocol;
    endpoints: ProviderEndpoints | null;
    error: unknown;
  } | null>(null);
  useEffect(() => {
    let active = true;
    setResolution(null);
    if (baseUrl && upstreamProtocol) {
      void resolveProviderEndpoints(baseUrl, upstreamProtocol, connection).then((endpoints) => {
        if (active) setResolution({ baseUrl, upstreamProtocol, endpoints, error: null });
      }).catch((caught: unknown) => {
        if (active) setResolution({ baseUrl, upstreamProtocol, endpoints: null, error: caught });
      });
    }
    return () => { active = false; };
  }, [baseUrl, upstreamProtocol, connection]);
  const current = resolution?.baseUrl === baseUrl && resolution.upstreamProtocol === upstreamProtocol
    ? resolution : null;
  return { endpoints: current?.endpoints ?? null, endpointError: current?.error == null ? null : errorText(current.error, t),
    resolvingEndpoint: Boolean(baseUrl && upstreamProtocol && !current) };
}

function sameModelConnection(current: ProviderConnectionInput, requested: ProviderConnectionInput): boolean {
  return current.app === requested.app && current.baseUrl?.trim() === requested.baseUrl?.trim()
    && current.upstreamProtocol === requested.upstreamProtocol && current.apiKey === requested.apiKey
    && current.authentication === requested.authentication && current.connection === requested.connection;
}

export function useProviderConnection(input: ProviderConnectionInput) {
  const { t } = useI18n();
  const [models, setModels] = useState<ProviderModel[] | null>(null);
  const [modelsBusy, setModelsBusy] = useState(false);
  const [modelsError, setModelsError] = useMessageState();
  const modelsVersion = useRef(0);
  const modelsPending = useRef<number | null>(null);
  const currentInput = useRef(input);
  currentInput.current = input;
  const isModelsPending = useCallback(() => modelsPending.current !== null, []);
  const baseUrl = input.baseUrl?.trim() ?? "";
  const endpoints = useResolvedEndpoints(baseUrl, input.upstreamProtocol, input.connection);
  const gatewayRouteWarning = useGatewayWarning(input);
  const modelsEndpointError = input.connection?.isFullUrl && !input.connection.modelsUrl?.trim()
    ? t("providers.gateway.modelsUrlMissing")
    : endpoints.endpoints?.modelsError ?? null;
  const modelAuthentication = input.authentication ?? undefined;
  const modelConnection = input.connection && Object.keys(input.connection).length > 0
    ? input.connection : undefined;
  useEffect(() => {
    modelsVersion.current += 1;
    modelsPending.current = null;
    setModels(null);
    setModelsError(null);
    setModelsBusy(false);
    return () => { modelsVersion.current += 1; modelsPending.current = null; };
  }, [baseUrl, input.app, input.upstreamProtocol, input.apiKey, input.authentication, input.connection]);

  /** Fetches the upstream model list; resolves with the fetched models, or
   * null when the request was skipped or superseded. */
  const fetchModels = async (): Promise<ProviderModel[] | null> => {
    if (isModelsPending() || !baseUrl || !input.upstreamProtocol) return null;
    if (modelsEndpointError) {
      setModelsError(input.connection?.isFullUrl && !input.connection.modelsUrl?.trim()
        ? uiMessage("providers.gateway.modelsUrlMissing") : modelsEndpointError);
      return null;
    }
    const version = modelsVersion.current;
    const current = () => modelsVersion.current === version && sameModelConnection(currentInput.current, input);
    modelsPending.current = version;
    setModelsBusy(true);
    setModelsError(null);
    try {
      const fetched = await fetchProviderModels(
        input.app,
        baseUrl,
        input.apiKey,
        input.upstreamProtocol,
        modelAuthentication,
        modelConnection,
      );
      if (current()) setModels(fetched);
      return current() ? fetched : null;
    } catch (caught) {
      if (current()) {
        setModelsError(caught);
      }
      return null;
    } finally {
      if (modelsVersion.current === version && modelsPending.current === version) {
        modelsPending.current = null;
        setModelsBusy(false);
      }
    }
  };
  return { models, modelsBusy, modelsError, modelsEndpointError, baseUrl, gatewayRouteWarning, fetchModels, isModelsPending, ...endpoints };
}
