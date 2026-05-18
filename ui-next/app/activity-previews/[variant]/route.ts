import { handlePreviewRequest } from "../route";

export const runtime = "nodejs";

export async function GET(
  request: Request,
  context: { params: Promise<{ variant: string }> },
) {
  const { variant } = await context.params;

  return handlePreviewRequest(
    request,
    variant === "thumbnail" ? "thumbnail" : "full",
  );
}
