/*
 * Please refer to https://docs.envio.dev for a thorough guide on all Envio indexer features
 */
import { AllEvents } from "generated";
import { expectType, TypeEqual } from "ts-expect";
import * as S from "rescript-schema";

type RemoveReadonly<T> = T extends {}
  ? {
      -readonly [key in keyof T]: RemoveReadonly<T[key]>;
    }
  : T;

type AssertSchemaType<Target, Schema> = TypeEqual<
  RemoveReadonly<Target>,
  S.Output<Schema>
>;

const SExtra = {
  void: S.undefined as S.Schema<void>,
  swayOptional: <T>(schema: S.Schema<T>) =>
    S.union([
      {
        case: "None" as const,
        payload: SExtra.void,
      },
      {
        case: "Some" as const,
        payload: schema,
      },
    ]),
  swayResult: <T, E>(ok: S.Schema<T>, err: S.Schema<E>) =>
    S.union([
      {
        case: "Ok" as const,
        payload: ok,
      },
      {
        case: "Err" as const,
        payload: err,
      },
    ]),
};

const unitLogSchema = SExtra.void;
AllEvents.UnitLog.handler(async ({ event }) => {
  S.assertWith(event.params, unitLogSchema)!;
  expectType<AssertSchemaType<typeof event.params, typeof unitLogSchema>>(true);
});

const optionLogSchema = SExtra.swayOptional(S.number);
// Add underscore here, because otherwise ReScript adds $$ which breaks runtime
AllEvents.Option_.handler(async ({ event }) => {
  S.assertWith(event.params, optionLogSchema)!;
  expectType<AssertSchemaType<typeof event.params, typeof optionLogSchema>>(
    true
  );
});

const simpleStructWithOptionalSchema = S.schema({
  f1: S.number,
  f2: SExtra.swayOptional(S.number),
});
AllEvents.SimpleStructWithOptionalField.handler(async ({ event }) => {
  S.assertWith(event.params, simpleStructWithOptionalSchema)!;
  expectType<
    AssertSchemaType<typeof event.params, typeof simpleStructWithOptionalSchema>
  >(true);
});

const u8LogSchema = S.number;
AllEvents.U8Log.handler(async ({ event }) => {
  S.assertWith(event.params, u8LogSchema)!;
  expectType<AssertSchemaType<typeof event.params, typeof u8LogSchema>>(true);
});

const u16LogSchema = S.number;
AllEvents.U16Log.handler(async ({ event }) => {
  S.assertWith(event.params, u16LogSchema)!;
  expectType<AssertSchemaType<typeof event.params, typeof u16LogSchema>>(true);
});

const u32LogSchema = S.number;
AllEvents.U32Log.handler(async ({ event }) => {
  S.assertWith(event.params, u32LogSchema)!;
  expectType<AssertSchemaType<typeof event.params, typeof u32LogSchema>>(true);
});

const u64LogSchema = S.bigint;
AllEvents.U64Log.handler(async ({ event }) => {
  S.assertWith(event.params, u64LogSchema)!;
  expectType<AssertSchemaType<typeof event.params, typeof u64LogSchema>>(true);
});

const b256LogSchema = S.string;
AllEvents.B256Log.handler(async ({ event }) => {
  S.assertWith(event.params, b256LogSchema)!;
  expectType<AssertSchemaType<typeof event.params, typeof b256LogSchema>>(true);
});

const arrayLogSchema = S.array(S.number);
AllEvents.ArrayLog.handler(async ({ event }) => {
  S.assertWith(event.params, arrayLogSchema)!;
  expectType<AssertSchemaType<typeof event.params, typeof arrayLogSchema>>(
    true
  );
});

const resultLogSchema = SExtra.swayResult(S.number, S.boolean);
AllEvents.Result.handler(async ({ event }) => {
  S.assertWith(event.params, resultLogSchema)!;
  expectType<AssertSchemaType<typeof event.params, typeof resultLogSchema>>(
    true
  );
});

const statusSchema = S.union([
  {
    case: "Pending" as const,
    payload: SExtra.void,
  },
  {
    case: "Completed" as const,
    payload: S.number,
  },
  {
    case: "Failed" as const,
    payload: {
      reason: S.number,
    },
  },
]);
AllEvents.Status.handler(async ({ event }) => {
  S.assertWith(event.params, statusSchema)!;
  expectType<AssertSchemaType<typeof event.params, typeof statusSchema>>(true);
});

const tupleLogSchema = S.tuple([S.bigint, S.boolean]);
AllEvents.TupleLog.handler(async ({ event }) => {
  S.assertWith(event.params, tupleLogSchema)!;
  expectType<AssertSchemaType<typeof event.params, typeof tupleLogSchema>>(
    true
  );
});

const simpleStructSchema = S.schema({
  f1: S.number,
});
AllEvents.SimpleStruct.handler(async ({ event }) => {
  S.assertWith(event.params, simpleStructSchema)!;
  expectType<AssertSchemaType<typeof event.params, typeof simpleStructSchema>>(
    true
  );
});

const unknownLogSchema = S.bigint;
AllEvents.UnknownLog.handler(async ({ event }) => {
  S.assertWith(event.params, unknownLogSchema)!;
  expectType<AssertSchemaType<typeof event.params, typeof unknownLogSchema>>(
    true
  );
});

const boolLogSchema = S.boolean;
AllEvents.BoolLog.handler(
  async ({ event }) => {
    S.assertWith(event.params, boolLogSchema)!;
    expectType<AssertSchemaType<typeof event.params, typeof boolLogSchema>>(
      true
    );
  },
  { wildcard: true }
);

const strLogSchema = S.string;
AllEvents.StrLog.handler(
  async ({ event }) => {
    S.assertWith(event.params, strLogSchema)!;
    expectType<AssertSchemaType<typeof event.params, typeof strLogSchema>>(
      true
    );
  },
  { wildcard: true }
);

const option2LogSchema = SExtra.swayOptional(SExtra.swayOptional(S.number));
AllEvents.Option2.handler(async ({ event }) => {
  S.assertWith(event.params, option2LogSchema)!;
  expectType<AssertSchemaType<typeof event.params, typeof option2LogSchema>>(
    true
  );
});

const vecLogSchema = S.array(S.bigint);
AllEvents.VecLog.handler(async ({ event }) => {
  S.assertWith(event.params, vecLogSchema)!;
  expectType<AssertSchemaType<typeof event.params, typeof vecLogSchema>>(true);
});

const bytesLogSchema = S.unknown;
AllEvents.BytesLog.handler(async ({ event }) => {
  S.assertWith(event.params, bytesLogSchema)!;
  expectType<AssertSchemaType<typeof event.params, typeof bytesLogSchema>>(
    true
  );
});

const mintSchema = S.schema({
  subId: S.string,
  amount: S.bigint,
});
AllEvents.Mint.handler(async ({ event }) => {
  S.assertWith(event.params, mintSchema)!;
  expectType<AssertSchemaType<typeof event.params, typeof mintSchema>>(true);
});

const burnSchema = S.schema({
  subId: S.string,
  amount: S.bigint,
});
AllEvents.Burn.handler(async ({ event }) => {
  S.assertWith(event.params, burnSchema)!;
  expectType<AssertSchemaType<typeof event.params, typeof burnSchema>>(true);
});

const transferOutSchema = S.schema({
  assetId: S.string,
  to: S.string,
  amount: S.bigint,
});
AllEvents.Transfer.handler(async ({ event }) => {
  S.assertWith(event.params, transferOutSchema)!;
  expectType<AssertSchemaType<typeof event.params, typeof transferOutSchema>>(
    true
  );
});

// const callSchema = S.object({
//   assetId: S.string,
//   to: S.string,
//   amount: S.bigint,
// });
// AllEvents.Call.handler(
//   async ({ event }) => {
//     S.assertWith(event.params,callSchema)!;
//     expectType<AssertSchemaType<typeof event.params, typeof callSchema>>(true);
//   }
//   { wildcard: true }
// );
