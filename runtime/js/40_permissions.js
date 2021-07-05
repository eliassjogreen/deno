// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const {
    Event,
    EventTarget,
    Deno: { core },
    __bootstrap: { webUtil: { illegalConstructorKey } },
  } = window;
  const {
    ArrayPrototypeIncludes,
    Map,
    MapPrototypeGet,
    MapPrototypeHas,
    MapPrototypeSet,
    FunctionPrototypeCall,
    PromiseResolve,
    PromiseReject,
    SymbolFor,
    TypeError,
  } = window.__bootstrap.primordials;

  /**
   * @typedef StatusCacheValue
   * @property {PermissionState} state
   * @property {PermissionStatus} status
   */

  /** @type {ReadonlyArray<"read" | "write" | "net" | "env" | "run" | "ffi" | "hrtime">} */
  const permissionNames = [
    "read",
    "write",
    "net",
    "env",
    "run",
    "ffi",
    "hrtime",
  ];

  /**
   * @param {Deno.PermissionDescriptor} desc
   * @returns {Deno.PermissionState}
   */
  function opQuery(desc) {
    return core.opSync("op_query_permission", desc);
  }

  /**
   * @param {Deno.PermissionDescriptor} desc
   * @returns {Deno.PermissionState}
   */
  function opRevoke(desc) {
    return core.opSync("op_revoke_permission", desc);
  }

  /**
   * @param {Deno.PermissionDescriptor} desc
   * @returns {Deno.PermissionState}
   */
  function opRequest(desc) {
    return core.opSync("op_request_permission", desc);
  }

  class PermissionStatus extends EventTarget {
    /** @type {{ state: Deno.PermissionState }} */
    #state;

    /** @type {((this: PermissionStatus, event: Event) => any) | null} */
    onchange = null;

    /** @returns {Deno.PermissionState} */
    get state() {
      return this.#state.state;
    }

    /**
     * @param {{ state: Deno.PermissionState }} state
     * @param {unknown} key
     */
    constructor(state = null, key = null) {
      if (key != illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }
      super();
      this.#state = state;
    }

    /**
     * @param {Event} event
     * @returns {boolean}
     */
    dispatchEvent(event) {
      let dispatched = super.dispatchEvent(event);
      if (dispatched && this.onchange) {
        FunctionPrototypeCall(this.onchange, this, event);
        dispatched = !event.defaultPrevented;
      }
      return dispatched;
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({ state: this.state, onchange: this.onchange })
      }`;
    }
  }

  /** @type {Map<string, StatusCacheValue>} */
  const statusCache = new Map();

  /**
   *
   * @param {Deno.PermissionDescriptor} desc
   * @param {Deno.PermissionState} state
   * @returns {PermissionStatus}
   */
  function cache(desc, state) {
    let { name: key } = desc;
    if ((desc.name === "read" || desc.name === "write") && "path" in desc) {
      key += `-${desc.path}`;
    } else if (desc.name === "net" && desc.host) {
      key += `-${desc.host}`;
    }
    if (MapPrototypeHas(statusCache, key)) {
      const status = MapPrototypeGet(statusCache, key);
      if (status.state !== state) {
        status.state = state;
        status.status.dispatchEvent(new Event("change", { cancelable: false }));
      }
      return status.status;
    }
    /** @type {{ state: Deno.PermissionState; status?: PermissionStatus }} */
    const status = { state };
    status.status = new PermissionStatus(status, illegalConstructorKey);
    MapPrototypeSet(statusCache, key, status);
    return status.status;
  }

  /**
   * @param {unknown} desc
   * @returns {desc is Deno.PermissionDescriptor}
   */
  function isValidDescriptor(desc) {
    return desc && desc !== null &&
      ArrayPrototypeIncludes(permissionNames, desc.name);
  }

  class Permissions {
    constructor(key = null) {
      if (key != illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }
    }

    query(desc) {
      if (!isValidDescriptor(desc)) {
        return PromiseReject(
          new TypeError(
            `The provided value "${desc.name}" is not a valid permission name.`,
          ),
        );
      }
      const state = opQuery(desc);
      return PromiseResolve(cache(desc, state));
    }

    revoke(desc) {
      if (!isValidDescriptor(desc)) {
        return PromiseReject(
          new TypeError(
            `The provided value "${desc.name}" is not a valid permission name.`,
          ),
        );
      }
      const state = opRevoke(desc);
      return PromiseResolve(cache(desc, state));
    }

    request(desc) {
      if (!isValidDescriptor(desc)) {
        return PromiseReject(
          new TypeError(
            `The provided value "${desc.name}" is not a valid permission name.`,
          ),
        );
      }
      const state = opRequest(desc);
      return PromiseResolve(cache(desc, state));
    }
  }

  const permissions = new Permissions(illegalConstructorKey);

  window.__bootstrap.permissions = {
    permissions,
    Permissions,
    PermissionStatus,
  };
})(this);
