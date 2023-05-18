// Generated by ReScript, PLEASE EDIT WITH CARE
"use strict";

const { benchmarkSuite } = require("jest-bench");
const IO = require("../../generated/src/IO.bs");
const Jest = require("@glennsl/rescript-jest/src/jest.bs.js");
const Js_dict = require("rescript/lib/js/js_dict.js");
const MockEvents = require("./../__mocks__/MockEvents.bs.js");
const ContextMock = require("./../__mocks__/ContextMock.bs.js");

require("../../src/EventHandlers.bs.js");

benchmarkSuite("Sample suite 1", {
        ["3 new gravatar event insert calls in order"]: () => {
                //Not a useful benchmark and calss are not mocked in Js
                // var insertCalls = Jest.MockJs.calls(ContextMock.insertMock);
                // return Jest.Expect.toEqual(Jest.Expect.expect(insertCalls), [
                //         MockEvents.newGravatar1.id.toString(),
                //         MockEvents.newGravatar2.id.toString(),
                //         MockEvents.newGravatar3.id.toString(),
                // ]);
        },

        ["Validate in memory store state"]: () => {
                var inMemoryStore = IO.InMemoryStore.Gravatar.gravatarDict.contents;
                var inMemoryStoreRows = Js_dict.values(inMemoryStore);
                return Jest.Expect.toEqual(Jest.Expect.expect(inMemoryStoreRows), [
                        {
                                crud: /* Update */ 2,
                                entity: {
                                        id: "1001",
                                        owner: "0x1230000000000000000000000000000000000000",
                                        displayName: "update1",
                                        imageUrl: "https://gravatar1.com",
                                        updatesCount: 2,
                                },
                        },
                        {
                                crud: /* Update */ 2,
                                entity: {
                                        id: "1002",
                                        owner: "0x4560000000000000000000000000000000000000",
                                        displayName: "update2",
                                        imageUrl: "https://gravatar2.com",
                                        updatesCount: 2,
                                },
                        },
                        {
                                crud: /* Create */ 0,
                                entity: {
                                        id: "1003",
                                        owner: "0x7890000000000000000000000000000000000000",
                                        displayName: "update3",
                                        imageUrl: "https://gravatar3.com",
                                        updatesCount: 2,
                                },
                        },
                ]);
        },
});