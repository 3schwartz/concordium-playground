import { IdStatement, IdProofOutput } from "@concordium/web-sdk";
import { VERIFIER_URL } from "../utils/config";

export async function getChallenge(account: string): Promise<string> {
    const response = await fetch(`${VERIFIER_URL}/challenge?address=${account}`, {method: "get"});
    const body = await response.json();
    return body.challenge;
}

export async function getStatement(): Promise<IdStatement> {
    const response = await fetch(`${VERIFIER_URL}/statement`,
    { method: "get" });
  const body = await response.json();
  return JSON.parse(body);
}

export async function getSignature(challenge: string, proof: IdProofOutput): Promise<string> {
    const response = await fetch(`${VERIFIER_URL}/prove`,
    {
      method: "post",
      headers: new Headers({ 'content-type': 'application/json' }),
      body: JSON.stringify({ challenge, proof })
    });
  const body = await response.json();
  return body;
}