
{{#each contracts as | contract |}}
let register{{contract.name.capitalized}}Handlers = () => {
try {
  let _ = %raw(`require("{{contract.handler.relative_to_generated_src}}")`)
  } catch {
  | _ => Js.log("Unable to find the handler file for {{contract.name}}. Please place a file at {{contract.handler.relative_to_generated_src}}")
  }
}

{{/each}}


let registerAllHandlers = () => {
{{#each contracts as | contract |}}
  register{{contract.name.capitalized}}Handlers()
{{/each}}
}
