import kaleido, { newQueryClient } from "@ericbutera/kaleido";
import { useQueryClient } from "@tanstack/react-query";
import toast from "react-hot-toast";
import { $api } from "./api";

export const queryClient = newQueryClient();

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
