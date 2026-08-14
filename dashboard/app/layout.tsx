import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "22Pie Remote",
  description: "Authorized remote screen viewing",
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return <html lang="en"><body>{children}</body></html>;
}
