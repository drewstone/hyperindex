// Generated by ReScript, PLEASE EDIT WITH CARE
'use strict';

var Curry = require("rescript/lib/js/curry.js");
var Ethers = require("generated/src/bindings/Ethers.bs.js");
var Handlers = require("generated/src/Handlers.bs.js");
var Belt_Option = require("rescript/lib/js/belt_Option.js");

Curry._1(Handlers.GravatarContract.NewGravatar.loader, (function ($$event, context) {
        Curry._1(context.contractRegistration.addSimpleNft, $$event.srcAddress);
      }));

Curry._1(Handlers.GravatarContract.NewGravatar.handler, (function ($$event, context) {
        Curry._1(context.gravatar.set, {
              id: $$event.params.id.toString(),
              owner: Ethers.ethAddressToString($$event.params.owner),
              displayName: $$event.params.displayName,
              imageUrl: $$event.params.imageUrl,
              updatesCount: BigInt(1)
            });
      }));

Curry._1(Handlers.GravatarContract.UpdatedGravatar.loader, (function ($$event, context) {
        Curry._2(context.gravatar.gravatarWithChangesLoad, $$event.params.id.toString(), {
              loadOwner: {}
            });
      }));

Curry._1(Handlers.GravatarContract.UpdatedGravatar.handler, (function ($$event, context) {
        var updatesCount = Belt_Option.mapWithDefault(Curry._1(context.gravatar.gravatarWithChanges, undefined), BigInt(1), (function (gravatar) {
                return Ethers.$$BigInt.add(gravatar.updatesCount, BigInt(1));
              }));
        Curry._1(context.gravatar.set, {
              id: $$event.params.id.toString(),
              owner: Ethers.ethAddressToString($$event.params.owner),
              displayName: $$event.params.displayName,
              imageUrl: $$event.params.imageUrl,
              updatesCount: updatesCount
            });
      }));

/*  Not a pure module */
