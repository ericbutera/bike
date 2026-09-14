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
const CURRENT_USER_QUERY_KEY = ["get", "/auth/current"] as const;
const CURRENT_USER_STALE_TIME_MS = 30 * 60 * 1000;
const CURRENT_USER_GC_TIME_MS = 60 * 60 * 1000;
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

function mapCurrentUser(rawUser: any) {
  return rawUser
    ? {
        id: rawUser.id ?? rawUser.pid,
        email: rawUser.email,
        name: rawUser.name,
        verified: rawUser.verified,
        is_admin: rawUser.is_admin,
      }
    : null;
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

const baseAuthApiClient = kaleido.createAuthApiClient();

async function refreshCurrentUserQuery(
  client: ReturnType<typeof useQueryClient>,
) {
  await Promise.all([
    client.invalidateQueries({ queryKey: CURRENT_USER_QUERY_KEY }),
    client.refetchQueries({ queryKey: CURRENT_USER_QUERY_KEY }),
  ]);
}

const tokenRefreshAuthApiClient = baseAuthApiClient.useTokenRefresh
  ? {
      useTokenRefresh() {
        const queryClient = useQueryClient();
        const mutation = baseAuthApiClient.useTokenRefresh!();

        return {
          ...mutation,
          mutateAsync: async () => {
            await mutation.mutateAsync();
            await refreshCurrentUserQuery(queryClient);
          },
        };
      },
    }
  : {};

export const authApiClient = {
  ...baseAuthApiClient,
  ...tokenRefreshAuthApiClient,
  useCurrentUser() {
    const response = $api.useQuery("get", "/auth/current", {
      options: {
        enabled: true,
        gcTime: CURRENT_USER_GC_TIME_MS,
        refetchOnMount: false,
        refetchOnReconnect: false,
        refetchOnWindowFocus: false,
        retry: false,
        staleTime: CURRENT_USER_STALE_TIME_MS,
      },
    });

    const isLoading = response.isLoading && !response.isError;
    const status = response.error?.response?.status;
    const rawUser =
      response.isError && status === 401 ? null : (response.data ?? null);

    return {
      user: mapCurrentUser(rawUser),
      isLoading,
      isError: response.isError,
    };
  },
  useVerifyEmail() {
    const queryClient = useQueryClient();
    const mutation = baseAuthApiClient.useVerifyEmail();

    return {
      ...mutation,
      mutateAsync: async (
        token: string,
        setError?: Parameters<typeof mutation.mutateAsync>[1],
      ) => {
        await mutation.mutateAsync(token, setError);
        await refreshCurrentUserQuery(queryClient);
      },
    };
  },
};
export const useAuth = kaleido.useAuth;
export default kaleido;
