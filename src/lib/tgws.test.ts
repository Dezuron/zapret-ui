import { describe, expect, it } from "vitest";
import {
  displayHost,
  isPublicBinding,
  tgwsEndpoint,
  tgwsLogTail,
  tgwsStateLabel,
  tgwsTone,
} from "./tgws";

describe("tgwsStateLabel", () => {
  it("describes every supervisor state", () => {
    expect(tgwsStateLabel("running")).toBe("Прокси включён");
    expect(tgwsStateLabel("stopped")).toBe("Прокси выключен");
    expect(tgwsStateLabel("not_installed")).toBe("Не установлен");
    expect(tgwsStateLabel(undefined)).toBe("Прокси выключен");
  });
});

describe("tgwsTone", () => {
  it("maps states to status colours", () => {
    expect(tgwsTone("running")).toBe("ok");
    expect(tgwsTone("starting")).toBe("warn");
    expect(tgwsTone("error")).toBe("danger");
    expect(tgwsTone("stopped")).toBe("idle");
  });
});

describe("displayHost", () => {
  it("turns wildcard bindings into loopback", () => {
    expect(displayHost("0.0.0.0")).toBe("127.0.0.1");
    expect(displayHost("::")).toBe("127.0.0.1");
    expect(displayHost("")).toBe("127.0.0.1");
    expect(displayHost("127.0.0.1")).toBe("127.0.0.1");
  });
});

describe("tgwsEndpoint", () => {
  it("renders host:port and brackets IPv6", () => {
    expect(tgwsEndpoint("127.0.0.1", 1443)).toBe("127.0.0.1:1443");
    expect(tgwsEndpoint("0.0.0.0", 1443)).toBe("127.0.0.1:1443");
    expect(tgwsEndpoint("::1", 1443)).toBe("[::1]:1443");
    expect(tgwsEndpoint(undefined, undefined)).toBe("127.0.0.1:1443");
  });
});

describe("isPublicBinding", () => {
  it("flags non-loopback hosts", () => {
    expect(isPublicBinding("127.0.0.1")).toBe(false);
    expect(isPublicBinding("localhost")).toBe(false);
    expect(isPublicBinding("192.168.1.10")).toBe(true);
  });
});

describe("tgwsLogTail", () => {
  it("keeps only the newest lines", () => {
    const lines = Array.from({ length: 350 }, (_, index) => `line-${index}`);
    const tail = tgwsLogTail(lines);
    expect(tail).toHaveLength(300);
    expect(tail[0]).toBe("line-50");
    expect(tail[299]).toBe("line-349");
    expect(tgwsLogTail(["a"])).toEqual(["a"]);
  });
});
