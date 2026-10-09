import { readFile } from 'node:fs/promises';
import { planResources } from '../resource-admission.mjs';

// Runs the public resolver under the same allowlisted disposable environment as
// Pi. No extension module or credential store is loaded by this driver.
const request = JSON.parse(await readFile(process.argv[2], 'utf8'));
process.stdout.write(JSON.stringify(await planResources(request)) + '\n');
