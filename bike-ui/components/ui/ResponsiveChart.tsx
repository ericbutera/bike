"use client";

import type { ComponentProps } from "react";
import { ResponsiveContainer } from "recharts";

// Give charts a positive viewport before ResizeObserver measures their container.
export default function ResponsiveChart(
  props: ComponentProps<typeof ResponsiveContainer>,
) {
  return (
    <ResponsiveContainer
      initialDimension={{ width: 320, height: 240 }}
      {...props}
    />
  );
}
