import kaleido, { handleApiError } from "@ericbutera/kaleido";
import {
  MutationCache,
  QueryCache,
  QueryClient,
  useQueryClient,
} from "@tanstack/react-query";
import toast from "react-hot-toast";
import { $api } from "./api";

const QUERY_ERROR_TOAST_THROTTLE_MS = 30_000;
const queryErrorToastTimes = new Map<string, number>();

function getHttpStatus(error: unknown) {
  if (typeof error === "object" && error && "response" in error) {
    const response = (error as { response?: { status?: unknown } }).response;
    return typeof response?.status === "number" ? response.status : null;
  }

  return null;
}

function showApiErrorToast(error: unknown) {
  const apiError = handleApiError(error);

  if (!apiError.errors && apiError.message) {
    toast.error(apiError.message);
  }

  console.error(`[API Error] ${apiError.message}`, apiError.errors);
}

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
      retry: 1,
      staleTime: 5 * 60 * 1000,
    },
  },
  queryCache: new QueryCache({
    onError: (error, query) => {
      if (getHttpStatus(error) === 401) {
        return;
      }

      const now = Date.now();
      const lastToastAt = queryErrorToastTimes.get(query.queryHash) ?? 0;

      if (now - lastToastAt < QUERY_ERROR_TOAST_THROTTLE_MS) {
        return;
      }

      queryErrorToastTimes.set(query.queryHash, now);
      showApiErrorToast(error);
    },
  }),
  mutationCache: new MutationCache({
    onError: showApiErrorToast,
  }),
});

kaleido.configure({
  auth: true,
  featureFlags: true,
  tasks: true,
  adminUsers: true,
  api: $api,
  useQueryClient,
  toast,
});

export const authApiClient = kaleido.createAuthApiClient();
export const useAuth = kaleido.useAuth;
export default kaleido;
