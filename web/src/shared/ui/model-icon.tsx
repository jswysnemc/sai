import { useState } from "react";

type ModelIconProps={model:string;size?:number};
const PROVIDERS:Array<[RegExp,string]>=[[/gpt|^o[0-9]|codex/i,"openai"],[/claude|opus|sonnet|haiku/i,"anthropic"],[/gemini|palm/i,"google"],[/deepseek/i,"deepseek"],[/qwen/i,"alibaba"],[/glm|zhipu/i,"zhipuai"],[/llama/i,"meta"],[/mistral|mixtral/i,"mistral"],[/grok/i,"xai"],[/kimi|moonshot/i,"moonshotai"]];

/** 使用 models.dev 供应商 SVG 渲染模型图标，失败时显示文字回退。 */
export function ModelIcon({model,size=16}:ModelIconProps){
  const [failed,setFailed]=useState(false);const provider=PROVIDERS.find(([pattern])=>pattern.test(model))?.[1];
  if(provider && !failed) return <img width={size} height={size} src={`https://models.dev/logos/${provider}.svg`} alt="" aria-hidden="true" onError={()=>setFailed(true)} style={{objectFit:"contain"}}/>;
  return <span aria-label={`模型 ${model}`} style={{display:"inline-grid",width:size,height:size,placeItems:"center",borderRadius:4,background:"var(--graphite-soft,#394244)",color:"#eef2f0",fontSize:Math.max(8,size*.48),fontWeight:600}}>{model.slice(0,2)}</span>;
}
