import {createClient} from "rrelayer";

export const client = createClient({
    serverUrl: 'http://localhost:8000',
    auth: {
        username: 'development_username_5r44TRuu',
        password: 'development_password_E4lRgmWB9qXq8mlF',
    },
});
